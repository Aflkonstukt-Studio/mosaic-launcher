use anyhow::{Result, anyhow};
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use log::{info, warn, error, debug};
use sha1::{Sha1, Digest};
use std::io::Read;
use crate::auth::AuthSession;
use crate::config::{Config, Profile};
use crate::file_manager::{FileManager, DownloadProgress};

// Minecraft version manifest URL
const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde(default)]
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDetails {
    pub id: String,
    pub r#type: String,
    pub time: String,
    #[serde(default)]
    pub release_time: String,
    pub main_class: Option<String>,
    pub minimum_launcher_version: Option<u32>,
    pub assets: String,
    pub assets_index: Option<AssetIndex>,
    pub downloads: HashMap<String, Download>,
    pub libraries: Vec<Library>,
    pub logging: Option<Logging>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
    pub total_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
    pub classifiers: Option<HashMap<String, Download>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<Os>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Os {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extract {
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    pub client: LoggingClient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub struct MinecraftManager {
    http_client: HttpClient,
    file_manager: FileManager,
    config: Config,
}

impl MinecraftManager {
    pub fn new(config: Config, file_manager: FileManager) -> Self {
        Self {
            http_client: HttpClient::new(),
            file_manager,
            config,
        }
    }

    // Helper method to get the Minecraft directory from the selected game
    fn get_minecraft_directory(&self) -> PathBuf {
        // Get the selected game ID
        let selected_game_id = self.config.selected_game.clone().unwrap_or_else(|| {
            if !self.config.games.is_empty() {
                self.config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game = self.config.games.iter()
            .find(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !self.config.games.is_empty() {
                    &self.config.games[0]
                } else {
                    panic!("No games found in config")
                }
            });

        game.game_directory.clone()
    }

    pub fn get_version_manifest(&self) -> Result<VersionManifest> {
        info!("Fetching Minecraft version manifest");
        let manifest = self.http_client
            .get(VERSION_MANIFEST_URL)
            .send()?
            .json::<VersionManifest>()?;

        Ok(manifest)
    }

    pub fn get_version_details(&self, version_info: &VersionInfo) -> Result<VersionDetails> {
        info!("Fetching details for Minecraft version {}", version_info.id);
        let details = self.http_client
            .get(&version_info.url)
            .send()?
            .json::<VersionDetails>()?;

        Ok(details)
    }

    pub fn is_version_installed(&self, version_id: &str) -> bool {
        let minecraft_dir = self.get_minecraft_directory();
        let version_jar = minecraft_dir.join("versions").join(version_id).join(format!("{}.jar", version_id));
        let version_json = minecraft_dir.join("versions").join(version_id).join(format!("{}.json", version_id));

        // Use std::fs instead of tokio::fs
        std::fs::metadata(&version_jar).is_ok() &&
            std::fs::metadata(&version_json).is_ok()
    }

    pub async fn download_version(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading Minecraft version {}", version_details.id);

        // Create version directory
        let minecraft_dir = self.get_minecraft_directory();
        let version_dir = minecraft_dir.join("versions").join(&version_details.id);
        self.file_manager.create_dir_all(&version_dir).await?;

        // Save version JSON
        let version_json_path = version_dir.join(format!("{}.json", version_details.id));
        let version_json = serde_json::to_string_pretty(version_details)?;
        fs::write(&version_json_path, version_json).await?;

        // Download client jar
        let client_download = version_details.downloads.get("client")
            .ok_or_else(|| anyhow!("Client download not found for version {}", version_details.id))?;

        let client_jar_path = version_dir.join(format!("{}.jar", version_details.id));
        self.file_manager.download_file(
            &client_download.url,
            &client_jar_path,
            Some(&client_download.sha1),
            progress_callback.clone(),
        ).await?;

        // Download logging configuration if available
        if let Some(logging) = &version_details.logging {
            self.download_logging_config(&logging.client, progress_callback.clone()).await?;
        }

        // Download libraries
        self.download_libraries(version_details, progress_callback.clone()).await?;

        // Download assets
        self.download_assets(version_details, progress_callback).await?;

        info!("Successfully downloaded Minecraft version {}", version_details.id);
        Ok(())
    }

    async fn download_logging_config(&self, logging_client: &LoggingClient, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static) -> Result<()> {
        let minecraft_dir = self.get_minecraft_directory();
        let assets_dir = minecraft_dir.join("assets");
        let log_configs_dir = assets_dir.join("log_configs");
        self.file_manager.create_dir_all(&log_configs_dir).await?;

        let log_config_path = log_configs_dir.join(&logging_client.file.id);
        self.file_manager.download_file(
            &logging_client.file.url,
            &log_config_path,
            Some(&logging_client.file.sha1),
            progress_callback,
        ).await?;

        Ok(())
    }

    async fn debug_native_jar(&self, jar_path: &Path) -> Result<()> {
        info!("Debugging native jar: {}", jar_path.display());

        let zip_data = tokio::fs::read(jar_path).await?;

        tokio::task::spawn_blocking(move || -> Result<()> {
            let cursor = std::io::Cursor::new(zip_data);
            let mut archive = zip::ZipArchive::new(cursor)?;

            info!("Jar contains {} entries:", archive.len());
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let name = file.name();
                let size = file.size();
                let is_dir = file.is_dir();
                info!("  {}: {} bytes ({})", name, size, if is_dir { "directory" } else { "file" });
            }

            Ok(())
        }).await??;

        Ok(())
    }

    fn debug_library_structure(&self, version_details: &VersionDetails) {
        info!("🔍 DEBUGGING LIBRARY STRUCTURE:");

        let mut lwjgl_libraries = Vec::new();
        let mut libraries_with_natives = Vec::new();
        let mut libraries_with_classifiers = Vec::new();

        for library in &version_details.libraries {
            // Collect LWJGL libraries specifically
            if library.name.contains("lwjgl") {
                lwjgl_libraries.push(library.name.clone());
            }

            // Check for natives
            if library.natives.is_some() {
                libraries_with_natives.push(library.name.clone());
            }

            // Check for classifiers
            if let Some(downloads) = &library.downloads {
                if downloads.classifiers.is_some() {
                    libraries_with_classifiers.push(library.name.clone());
                }
            }
        }

        info!("📦 LWJGL Libraries found ({} total):", lwjgl_libraries.len());
        for lib in &lwjgl_libraries {
            info!("  - {}", lib);
        }

        info!("🏠 Libraries with natives section ({} total):", libraries_with_natives.len());
        for lib in &libraries_with_natives {
            info!("  - {}", lib);
        }

        info!("🔧 Libraries with classifiers section ({} total):", libraries_with_classifiers.len());
        for lib in &libraries_with_classifiers {
            info!("  - {}", lib);
        }

        // Let's also look at a specific LWJGL library in detail
        for library in &version_details.libraries {
            if library.name.contains("lwjgl") && library.name.contains("glfw") && !library.name.contains("natives") {
                info!("🔍 Examining library in detail: {}", library.name);
                info!("  - Has natives: {}", library.natives.is_some());
                info!("  - Has downloads: {}", library.downloads.is_some());

                if let Some(downloads) = &library.downloads {
                    info!("  - Has artifact: {}", downloads.artifact.is_some());
                    info!("  - Has classifiers: {}", downloads.classifiers.is_some());

                    if let Some(classifiers) = &downloads.classifiers {
                        info!("  - Classifier keys: {:?}", classifiers.keys().collect::<Vec<_>>());
                    }
                }

                if let Some(natives) = &library.natives {
                    info!("  - Native mappings: {:?}", natives);
                }

                break; // Just examine one
            }
        }
    }

    async fn download_libraries(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading libraries for Minecraft version {}", version_details.id);
        info!("Total libraries in manifest: {}", version_details.libraries.len());

        let minecraft_dir = self.get_minecraft_directory();
        let libraries_dir = minecraft_dir.join("libraries");
        self.file_manager.create_dir_all(&libraries_dir).await?;

        // Create natives directory
        let natives_dir = minecraft_dir.join("versions")
            .join(&version_details.id)
            .join("natives");
        self.file_manager.create_dir_all(&natives_dir).await?;

        let mut native_libraries_found = 0;
        let mut native_libraries_extracted = 0;
        let mut total_libraries_processed = 0;
        let mut libraries_with_rules_skipped = 0;

        let os_name = self.get_os_name();
        let arch = self.get_arch();
        info!("Detected OS: {}, Architecture: {}", os_name, arch);

        for (index, library) in version_details.libraries.iter().enumerate() {
            info!("=== Processing library {}/{}: {} ===", index + 1, version_details.libraries.len(), library.name);

            // Check if this is a native library based on the name (new format)
            let is_separate_native_library = library.name.contains(":natives-");

            // Check rules to see if this library should be downloaded
            if let Some(rules) = &library.rules {
                if !self.should_use_library(rules) {
                    libraries_with_rules_skipped += 1;
                    if is_separate_native_library {
                        info!("❌ Skipping NATIVE library {} due to rules (probably wrong platform)", library.name);
                    } else {
                        debug!("❌ Skipping library {} due to rules", library.name);
                    }
                    continue;
                } else {
                    if is_separate_native_library {
                        info!("✅ NATIVE library {} passes rule check", library.name);
                    } else {
                        debug!("✅ Library {} passes rule check", library.name);
                    }
                }
            }

            total_libraries_processed += 1;

            // Check if library has downloads
            if let Some(downloads) = &library.downloads {
                // Download the main artifact if it exists
                if let Some(artifact) = &downloads.artifact {
                    let path = if is_separate_native_library {
                        // For native libraries, create a unique path that includes the full classifier
                        let parts: Vec<&str> = library.name.split(':').collect();
                        if parts.len() >= 4 {
                            let group_path = parts[0].replace('.', "/");
                            let artifact_id = parts[1];
                            let version = parts[2];
                            let classifier = parts[3]; // e.g., "natives-linux"

                            libraries_dir.join(&group_path)
                                .join(artifact_id)
                                .join(version)
                                .join(format!("{}-{}-{}.jar", artifact_id, version, classifier))
                        } else {
                            libraries_dir.join(self.get_library_path(&library.name))
                        }
                    } else {
                        libraries_dir.join(self.get_library_path(&library.name))
                    };

                    self.file_manager.create_dir_all(path.parent().unwrap()).await?;

                    self.file_manager.download_file(
                        &artifact.url,
                        &path,
                        Some(&artifact.sha1),
                        progress_callback.clone(),
                    ).await?;

                    // If this is a separate native library entry, extract it
                    if is_separate_native_library {
                        native_libraries_found += 1;

                        info!("🔧 Extracting separate native library: {}", library.name);

                        let extract_excludes = vec!["META-INF/".to_string()];
                        match self.file_manager.extract_zip(&path, &natives_dir, Some(&extract_excludes)).await {
                            Ok(_) => {
                                native_libraries_extracted += 1;
                                info!("✅ Successfully extracted native library: {}", library.name);
                            },
                            Err(e) => {
                                error!("❌ Failed to extract native library {}: {}", library.name, e);
                            }
                        }
                    }
                }

                // Handle old-style native libraries with classifiers (for older MC versions)
                if let (Some(classifiers), Some(natives)) = (&downloads.classifiers, &library.natives) {
                    info!("🎯 Processing OLD-STYLE native library: {}", library.name);

                    if let Some(native_key_template) = natives.get(os_name) {
                        let native_key = native_key_template.replace("${arch}", arch);
                        info!("🔍 Looking for classifier: {}", native_key);

                        if let Some(native_download) = classifiers.get(&native_key) {
                            native_libraries_found += 1;

                            // Build path for old-style native library
                            let library_parts: Vec<&str> = library.name.split(':').collect();
                            if library_parts.len() >= 3 {
                                let group_path = library_parts[0].replace('.', "/");
                                let artifact_id = library_parts[1];
                                let version = library_parts[2];

                                let native_jar_name = format!("{}-{}-{}.jar", artifact_id, version, native_key);
                                let native_jar_path = libraries_dir
                                    .join(&group_path)
                                    .join(artifact_id)
                                    .join(version)
                                    .join(native_jar_name);

                                self.file_manager.create_dir_all(native_jar_path.parent().unwrap()).await?;

                                info!("⬇️ Downloading old-style native jar: {}", native_download.url);
                                self.file_manager.download_file(
                                    &native_download.url,
                                    &native_jar_path,
                                    Some(&native_download.sha1),
                                    progress_callback.clone(),
                                ).await?;

                                // Extract old-style native library
                                let extract_excludes = if let Some(extract) = &library.extract {
                                    extract.exclude.clone()
                                } else {
                                    vec!["META-INF/".to_string()]
                                };

                                match self.file_manager.extract_zip(&native_jar_path, &natives_dir, Some(&extract_excludes)).await {
                                    Ok(_) => {
                                        native_libraries_extracted += 1;
                                        info!("✅ Successfully extracted old-style native library: {}", library.name);
                                    },
                                    Err(e) => {
                                        error!("❌ Failed to extract old-style native library {}: {}", library.name, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        info!("📊 SUMMARY:");
        info!("  Total libraries in manifest: {}", version_details.libraries.len());
        info!("  Libraries skipped due to rules: {}", libraries_with_rules_skipped);
        info!("  Libraries processed: {}", total_libraries_processed);
        info!("  Native libraries found: {}", native_libraries_found);
        info!("  Native libraries extracted: {}", native_libraries_extracted);

        // Final verification
        let native_files = std::fs::read_dir(&natives_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);

        info!("🔍 Final native files count in directory: {}", native_files);

        if native_files == 0 {
            warn!("⚠️ No native libraries found - this might be normal for very old Minecraft versions");
            // Don't fail for old versions that might not need native libraries
        }

        info!("✅ Successfully downloaded libraries for Minecraft version {}", version_details.id);
        Ok(())
    }

    async fn download_assets(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading assets for Minecraft version {}", version_details.id);

        let minecraft_dir = self.get_minecraft_directory();
        let assets_dir = minecraft_dir.join("assets");
        self.file_manager.create_dir_all(&assets_dir).await?;

        // Special case for versions with known missing asset indexes
        let known_missing_assets = ["24", "legacy", "1.6.4", "1.7.10"];
        let asset_index_id = version_details.assets_index.as_ref().map(|a| a.id.as_str())
            .unwrap_or_else(|| &version_details.assets);

        if known_missing_assets.contains(&asset_index_id) {
            warn!("Known missing asset index detected ({}). Creating minimal assets structure.", asset_index_id);

            // Create minimal assets structure
            let objects_dir = assets_dir.join("objects");
            self.file_manager.create_dir_all(&objects_dir).await?;

            // Create empty index file to prevent future download attempts
            let indexes_dir = assets_dir.join("indexes");
            self.file_manager.create_dir_all(&indexes_dir).await?;
            let index_path = indexes_dir.join(format!("{}.json", asset_index_id));

            // Create empty asset index
            let empty_index = AssetObjects { objects: HashMap::new() };
            let empty_index_json = serde_json::to_string(&empty_index)?;
            fs::write(&index_path, empty_index_json).await?;

            info!("Created minimal assets structure for version {}", version_details.id);
            return Ok(());
        }

        // Download asset index
        let indexes_dir = assets_dir.join("indexes");
        self.file_manager.create_dir_all(&indexes_dir).await?;

        // Handle both new and old asset index formats
        let (asset_index_id, asset_index_url, asset_index_sha1) = if let Some(assets_index) = &version_details.assets_index {
            // New format - has full AssetIndex object
            info!("Using assets_index object: {}", assets_index.id);
            (assets_index.id.clone(), assets_index.url.clone(), assets_index.sha1.clone())
        } else {
            // Old format - just has assets field with index name
            let asset_index_id = version_details.assets.clone();
            info!("Using assets field as index name: {}", asset_index_id);

            // Check if this is a very old version that might not have assets
            if asset_index_id == "legacy" || asset_index_id.parse::<f32>().unwrap_or(0.0) < 13.0 {
                warn!("Very old Minecraft version detected ({}). Assets may not be available or needed.", asset_index_id);

                // For very old versions, try to continue without assets
                // Create an empty assets directory structure
                let objects_dir = assets_dir.join("objects");
                self.file_manager.create_dir_all(&objects_dir).await?;

                info!("Skipping asset download for very old version {}", version_details.id);
                return Ok(());
            }

            // For newer old versions, construct modern URL
            let asset_index_url = format!("https://launchermeta.mojang.com/v1/packages/{}/{}.json", asset_index_id, asset_index_id);
            info!("Constructed asset index URL: {}", asset_index_url);
            (asset_index_id, asset_index_url, String::new())
        };

        let asset_index_path = indexes_dir.join(format!("{}.json", asset_index_id));

        // Try to download asset index with multiple fallback strategies
        let asset_objects = match self.try_download_asset_index(&asset_index_id, &asset_index_url, &asset_index_path, &asset_index_sha1, progress_callback.clone()).await {
            Ok(objects) => objects,
            Err(e) => {
                // For old versions, warn but don't fail
                if asset_index_id.parse::<f32>().unwrap_or(0.0) < 16.0 {
                    warn!("Could not download assets for old version {} ({}). Game may still work without them.", version_details.id, e);
                    let objects_dir = assets_dir.join("objects");
                    self.file_manager.create_dir_all(&objects_dir).await?;
                    return Ok(());
                } else {
                    return Err(e);
                }
            }
        };

        info!("Asset index contains {} objects", asset_objects.objects.len());

        // Download assets
        let objects_dir = assets_dir.join("objects");
        self.file_manager.create_dir_all(&objects_dir).await?;

        let mut downloaded_count = 0;
        let total_assets = asset_objects.objects.len();

        for (asset_name, asset) in &asset_objects.objects {
            let hash_prefix = &asset.hash[0..2];
            let asset_dir = objects_dir.join(hash_prefix);
            self.file_manager.create_dir_all(&asset_dir).await?;

            let asset_path = asset_dir.join(&asset.hash);

            // Skip if already exists with correct size
            if asset_path.exists() {
                if let Ok(metadata) = std::fs::metadata(&asset_path) {
                    if metadata.len() == asset.size {
                        continue;
                    }
                }
            }

            let asset_url = format!("https://resources.download.minecraft.net/{}/{}", hash_prefix, asset.hash);

            match self.file_manager.download_file(
                &asset_url,
                &asset_path,
                Some(&asset.hash),
                progress_callback.clone(),
            ).await {
                Ok(_) => downloaded_count += 1,
                Err(e) => {
                    warn!("Failed to download asset {}: {}", asset_name, e);
                    // Continue with other assets instead of failing completely
                }
            }

            if downloaded_count % 100 == 0 && downloaded_count > 0 {
                info!("Downloaded {}/{} assets", downloaded_count, total_assets);
            }
        }

        info!("Successfully downloaded {} assets for Minecraft version {}", downloaded_count, version_details.id);
        Ok(())
    }

    // Add this helper method
    async fn try_download_asset_index(
        &self,
        asset_index_id: &str,
        primary_url: &str,
        asset_index_path: &Path,
        asset_index_sha1: &str,
        progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
    ) -> Result<AssetObjects> {
        // List of URLs to try
        let urls_to_try = vec![
            primary_url.to_string(),
            format!("https://launchermeta.mojang.com/v1/packages/{}/{}.json", asset_index_id, asset_index_id),
            format!("https://piston-meta.mojang.com/v1/packages/{}/{}.json", asset_index_id, asset_index_id),
            // For some versions, the asset index might be available without the duplicate ID
            format!("https://launchermeta.mojang.com/v1/packages/{}.json", asset_index_id),
            format!("https://piston-meta.mojang.com/v1/packages/{}.json", asset_index_id),
        ];

        let mut last_error = None;

        for (i, url) in urls_to_try.iter().enumerate() {
            info!("Trying asset index URL {}/{}: {}", i + 1, urls_to_try.len(), url);

            // Only use SHA1 verification for the primary URL
            let sha1_ref = if i == 0 && !asset_index_sha1.is_empty() {
                Some(asset_index_sha1)
            } else {
                None
            };

            match self.file_manager.download_file(url, asset_index_path, sha1_ref, progress_callback.clone()).await {
                Ok(_) => {
                    // Try to parse the downloaded file
                    match fs::read_to_string(asset_index_path).await {
                        Ok(content) => {
                            // Check if it's actually JSON (not an error page)
                            if content.trim_start().starts_with('{') {
                                match serde_json::from_str::<AssetObjects>(&content) {
                                    Ok(objects) => {
                                        info!("Successfully downloaded and parsed asset index from: {}", url);
                                        return Ok(objects);
                                    },
                                    Err(e) => {
                                        warn!("Downloaded asset index from {} but failed to parse JSON: {}", url, e);
                                        last_error = Some(anyhow!("JSON parse error: {}", e));
                                        continue;
                                    }
                                }
                            } else {
                                warn!("Downloaded asset index from {} but got non-JSON response: {}", url, &content[..200.min(content.len())]);
                                last_error = Some(anyhow!("Non-JSON response"));
                                continue;
                            }
                        },
                        Err(e) => {
                            warn!("Downloaded asset index from {} but failed to read file: {}", url, e);
                            last_error = Some(anyhow!("File read error: {}", e));
                            continue;
                        }
                    }
                },
                Err(e) => {
                    warn!("Failed to download asset index from {}: {}", url, e);
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All asset index download attempts failed")))
    }

    pub async fn launch_game(&self, profile: &Profile, auth_session: &AuthSession, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Launching Minecraft with profile: {}", profile.name);

        // Validate Java installation
        self.validate_java_installation()?;

        // Get version details
        let version_manifest = self.get_version_manifest()?;
        let version_info = version_manifest.versions.iter()
            .find(|v| v.id == profile.version)
            .ok_or_else(|| anyhow!("Version {} not found in manifest", profile.version))?;

        let version_details = self.get_version_details(version_info)?;

        // Check if version is installed, if not, download it
        if !self.is_version_installed(&profile.version) {
            info!("Version {} is not installed. Downloading it first...", profile.version);
            self.download_version(&version_details, progress_callback).await?;
        }

        // Verify natives were extracted
        let natives_dir = self.get_minecraft_directory()
            .join("versions")
            .join(&version_details.id)
            .join("natives");

        info!("Verifying native libraries in: {}", natives_dir.display());
        let native_files = std::fs::read_dir(&natives_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);

        if native_files == 0 {
            warn!("No native libraries found - attempting to re-extract");
            self.force_extract_natives(&version_details).await?;
        }

        // Build Java command
        let mut command = Command::new(self.get_java_path()?);
        command.current_dir(&self.get_minecraft_directory());

        // Memory settings - use profile-specific memory if set, otherwise use global setting
        let memory = profile.memory.unwrap_or(self.config.max_memory);
        command.arg(format!("-Xmx{}M", memory));

        // Performance JVM args
        command.arg("-XX:+UseG1GC");
        command.arg("-XX:+ParallelRefProcEnabled");

        // Native library paths
        command.arg(format!("-Djava.library.path={}", natives_dir.display()));
        command.arg(format!("-Dorg.lwjgl.librarypath={}", natives_dir.display()));

        // Debugging (remove in production)
        command.arg("-Dorg.lwjgl.util.Debug=true");
        command.arg("-Dorg.lwjgl.util.DebugLoader=true");

        // Build classpath - ensure jopt-simple is included
        let classpath = self.build_classpath(&version_details)?;
        command.arg("-cp").arg(classpath);

        // Main class
        command.arg(version_details.main_class.as_deref().unwrap_or("net.minecraft.client.main.Main"));

        // Game arguments
        if let Some(arguments) = &version_details.arguments {
            for arg in &arguments.game {
                if let serde_json::Value::String(arg_str) = arg {
                    let arg_str = self.replace_placeholders(arg_str, profile, auth_session, &version_details)?;
                    command.arg(arg_str);
                }
            }
        } else if let Some(args) = &version_details.minecraft_arguments {
            for arg in args.split(' ') {
                command.arg(self.replace_placeholders(arg, profile, auth_session, &version_details)?);
            }
        }

        // Logging config
        if let Some(logging) = &version_details.logging {
            let log_path = self.get_minecraft_directory()
                .join("assets")
                .join("log_configs")
                .join(&logging.client.file.id);

            if log_path.exists() {
                command.arg(logging.client.argument.replace("${path}", &log_path.display().to_string()));
            }
        }

        // Launch
        info!("Executing: {:?}", command);
        let child = command
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        info!("Launched with PID: {}", child.id());
        Ok(())
    }

    async fn force_extract_natives(&self, version_details: &VersionDetails) -> Result<()> {
        let minecraft_dir = self.get_minecraft_directory();
        let libraries_dir = minecraft_dir.join("libraries");
        let natives_dir = minecraft_dir.join("versions").join(&version_details.id).join("natives");

        // Clear existing (potentially corrupt) natives
        let _ = std::fs::remove_dir_all(&natives_dir);
        std::fs::create_dir_all(&natives_dir)?;

        // Re-extract all native libraries
        for library in &version_details.libraries {
            if let Some(downloads) = &library.downloads {
                if let (Some(classifiers), Some(natives)) = (&downloads.classifiers, &library.natives) {
                    let os_name = self.get_os_name();
                    if let Some(native_key) = natives.get(os_name) {
                        let native_key = native_key.replace("${arch}", &self.get_arch());
                        if let Some(_native_download) = classifiers.get(&native_key) {
                            // Build correct path to the native jar
                            let library_parts: Vec<&str> = library.name.split(':').collect();
                            let artifact_id = library_parts.get(1).unwrap_or(&library_parts[0]);
                            let version = library_parts.get(2).unwrap_or(&"unknown");

                            let native_jar_name = format!("{}-{}-natives-{}.jar", artifact_id, version, os_name);
                            let native_jar_path = libraries_dir.join(self.get_library_path(&library.name))
                                .with_file_name(native_jar_name);

                            if native_jar_path.exists() {
                                info!("Re-extracting native library: {}", native_jar_path.display());
                                self.file_manager.extract_zip(&native_jar_path, &natives_dir, None).await?;
                            } else {
                                warn!("Native jar not found: {}", native_jar_path.display());
                            }
                        }
                    }
                }
            }
        }

        // Verify extraction worked
        let native_files = std::fs::read_dir(&natives_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);

        if native_files == 0 {
            return Err(anyhow!("Failed to extract any native libraries"));
        }

        info!("Successfully re-extracted {} native libraries", native_files);
        Ok(())
    }

    fn validate_java_installation(&self) -> Result<()> {
        let java_path = self.get_java_path()?;

        // Test if Java is executable and get version
        let output = Command::new(&java_path)
            .arg("-version")
            .output();

        match output {
            Ok(output) => {
                if !output.status.success() {
                    return Err(anyhow!("Java is installed but returned error code: {}", output.status));
                }

                let version_info = String::from_utf8_lossy(&output.stderr);
                info!("Java found: {}", version_info.lines().next().unwrap_or("Unknown version"));
                Ok(())
            },
            Err(e) => {
                Err(anyhow!("Java not found or not executable at path '{}': {}. Please install Java or specify the correct path in launcher settings.", java_path.display(), e))
            }
        }
    }

    fn build_classpath(&self, version_details: &VersionDetails) -> Result<String> {
        let minecraft_dir = self.get_minecraft_directory();
        let mut classpath = Vec::new();

        // Add all libraries
        for library in &version_details.libraries {
            if !self.should_use_library(&library.rules.clone().unwrap_or_default()) {
                debug!("Excluding library from classpath: {}", library.name);
                continue;
            }

            let path = minecraft_dir.join("libraries").join(self.get_library_path(&library.name));
            if path.exists() {
                debug!("Adding library to classpath: {}", library.name);
                classpath.push(path.display().to_string());
            } else {
                warn!("Library jar not found: {} (path: {})", library.name, path.display());
            }
        }

        // Add client jar
        let client_jar = minecraft_dir.join("versions")
            .join(&version_details.id)
            .join(format!("{}.jar", version_details.id));
        classpath.push(client_jar.display().to_string());

        info!("Built classpath with {} entries", classpath.len());
        Ok(classpath.join(if cfg!(windows) { ";" } else { ":" }))
    }

    fn get_java_path(&self) -> Result<PathBuf> {
        if let Some(java_path) = &self.config.java_path {
            return Ok(java_path.clone());
        }

        // Try to find Java in PATH
        let java_command = if cfg!(target_os = "windows") {
            "javaw.exe"
        } else {
            "java"
        };

        Ok(PathBuf::from(java_command))
    }

    fn get_library_path(&self, name: &str) -> PathBuf {
        let parts: Vec<&str> = name.split(':').collect();
        if parts.len() < 3 {
            return PathBuf::from(name);
        }

        let group_id = parts[0].replace('.', "/");
        let artifact_id = parts[1];
        let version = parts[2];

        PathBuf::from(format!("{}/{}/{}/{}-{}.jar", group_id, artifact_id, version, artifact_id, version))
    }

    fn should_use_library(&self, rules: &[Rule]) -> bool {
        // If no rules are specified, allow the library by default
        if rules.is_empty() {
            return true;
        }

        let mut allow = true; // Default to allow, not disallow

        for rule in rules {
            let matches = match &rule.os {
                Some(os) => {
                    let os_name_matches = match &os.name {
                        Some(name) => self.get_os_name() == name,
                        None => true,
                    };

                    let os_version_matches = match &os.version {
                        Some(_version) => {
                            // This would need a proper regex match in a real implementation
                            // For now, assume it matches
                            true
                        },
                        None => true,
                    };

                    let os_arch_matches = match &os.arch {
                        Some(arch) => self.get_arch() == arch,
                        None => true,
                    };

                    os_name_matches && os_version_matches && os_arch_matches
                },
                None => true,
            };

            if matches {
                allow = rule.action == "allow";
            }
        }

        allow
    }

    fn get_os_name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        }
    }

    fn get_arch(&self) -> &'static str {
        if cfg!(target_arch = "x86_64") {
            "64"
        } else {
            "32"
        }
    }

    // Check if user namespaces are enabled on Linux
    fn are_user_namespaces_enabled(&self) -> bool {
        if !cfg!(target_os = "linux") {
            return true; // Not relevant on non-Linux platforms
        }

        // Check if user namespaces are enabled in the kernel
        match std::fs::File::open("/proc/sys/kernel/unprivileged_userns_clone") {
            Ok(mut file) => {
                let mut contents = String::new();
                if file.read_to_string(&mut contents).is_ok() {
                    contents.trim() == "1"
                } else {
                    // If we can't read the file, assume they're enabled
                    true
                }
            },
            Err(_) => {
                // If the file doesn't exist, it might mean user namespaces are always enabled
                // or the system uses a different mechanism to control them
                true
            }
        }
    }

    // Get guidance for enabling user namespaces
    fn get_user_namespace_guidance(&self) -> String {
        if !cfg!(target_os = "linux") {
            return String::new(); // Not relevant on non-Linux platforms
        }

        let mut guidance = String::from(
            "To enable user namespaces on your system, you can try one of the following:\n\n"
        );

        // Check for common Linux distributions and provide specific guidance
        if std::path::Path::new("/etc/debian_version").exists() {
            // Debian/Ubuntu
            guidance.push_str(
                "For Debian/Ubuntu:\n\
                 1. Run: sudo sysctl -w kernel.unprivileged_userns_clone=1\n\
                 2. To make it permanent, add 'kernel.unprivileged_userns_clone=1' to /etc/sysctl.conf\n\n"
            );
        } else if std::path::Path::new("/etc/fedora-release").exists() {
            // Fedora
            guidance.push_str(
                "For Fedora:\n\
                 1. Run: sudo sysctl -w user.max_user_namespaces=15000\n\
                 2. To make it permanent, add 'user.max_user_namespaces=15000' to /etc/sysctl.conf\n\n"
            );
        } else if std::path::Path::new("/etc/arch-release").exists() {
            // Arch Linux
            guidance.push_str(
                "For Arch Linux:\n\
                 1. Run: sudo sysctl -w kernel.unprivileged_userns_clone=1\n\
                 2. To make it permanent, add 'kernel.unprivileged_userns_clone=1' to /etc/sysctl.d/99-sysctl.conf\n\n"
            );
        }

        // General guidance for all distributions
        guidance.push_str(
            "General solution:\n\
             1. You can run the launcher with sudo (not recommended for security reasons)\n\
             2. Or add the --no-sandbox flag to the Java command line arguments in the launcher settings\n\
             3. Or disable sandboxing in your system settings if available\n"
        );

        guidance
    }

    fn replace_placeholders(&self, arg: &str, profile: &Profile, auth_session: &AuthSession, version_details: &VersionDetails) -> Result<String> {
        let minecraft_profile = auth_session.minecraft_profile.as_ref()
            .ok_or_else(|| anyhow!("No Minecraft profile in auth session"))?;

        let minecraft_dir = self.get_minecraft_directory();
        let game_dir = match &profile.game_directory {
            Some(dir) => dir.clone(),
            None => minecraft_dir.clone(),
        };

        // Determine user type based on whether this is an offline session
        let user_type = if auth_session.is_offline { "offline" } else { "msa" };

        // Define the natives directory
        let natives_dir = minecraft_dir.join("versions").join(&version_details.id).join("natives");

        // Define launcher name and version
        let launcher_name = "Mosaic Launcher";
        let launcher_version = "0.1.0"; // Update with actual version

        // Define client ID and Xbox user ID
        let client_id = "mosaic-launcher"; // Placeholder value
        let auth_xuid = if auth_session.is_offline { "" } else { "0" }; // Placeholder value for non-offline sessions

        // Get assets index name
        let assets_index_name = if let Some(assets_index) = &version_details.assets_index {
            assets_index.id.clone()
        } else {
            version_details.assets.clone() // Fallback to assets field
        };

        let assets_root = if version_details.assets_index.is_none() &&
            ["24", "legacy", "1.6.4", "1.7.10"].contains(&version_details.assets.as_str()) {
            // Use a fallback assets directory
            minecraft_dir.join("assets").to_string_lossy().to_string()
        } else {
            minecraft_dir.join("assets").to_string_lossy().to_string()
        };
        
        let arg = arg.replace("${auth_player_name}", &minecraft_profile.name)
            .replace("${version_name}", &version_details.id)
            .replace("${game_directory}", &game_dir.to_string_lossy())
            .replace("${assets_index_name}", &assets_index_name)
            .replace("${auth_uuid}", &minecraft_profile.id)
            .replace("${auth_access_token}", &auth_session.minecraft_token.clone().unwrap_or_else(|| "offline".to_string()))
            .replace("${user_type}", user_type)
            .replace("${version_type}", &version_details.r#type)
            .replace("${natives_directory}", &natives_dir.to_string_lossy())
            .replace("${launcher_name}", launcher_name)
            .replace("${launcher_version}", launcher_version)
            .replace("${clientid}", client_id)
            .replace("${auth_xuid}", auth_xuid)
            .replace("${assets_root}", &assets_root)
            // Additional common placeholders
            .replace("${library_directory}", &minecraft_dir.join("libraries").to_string_lossy())
            .replace("${classpath_separator}", if cfg!(target_os = "windows") { ";" } else { ":" })
            // Remove problematic placeholders
            .replace("${classpath}", ""); // Just remove it since we build classpath separately

        Ok(arg)
    }
}
