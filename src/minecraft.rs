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
    pub main_class: String,
    pub minimum_launcher_version: u32,
    pub assets: String,
    pub assets_index: AssetIndex,
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

    pub async fn download_version(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading Minecraft version {}", version_details.id);

        // Create version directory
        let version_dir = self.config.minecraft_directory.join("versions").join(&version_details.id);
        self.file_manager.create_dir_all(&version_dir).await?;

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

        // Download libraries
        self.download_libraries(version_details, progress_callback.clone()).await?;

        // Download assets
        self.download_assets(version_details, progress_callback).await?;

        info!("Successfully downloaded Minecraft version {}", version_details.id);
        Ok(())
    }

    async fn download_libraries(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading libraries for Minecraft version {}", version_details.id);

        let libraries_dir = self.config.minecraft_directory.join("libraries");
        self.file_manager.create_dir_all(&libraries_dir).await?;

        for library in &version_details.libraries {
            // Check rules to see if this library should be downloaded
            if let Some(rules) = &library.rules {
                if !self.should_use_library(rules) {
                    continue;
                }
            }

            // Download the main artifact if it exists
            if let Some(downloads) = &library.downloads {
                if let Some(artifact) = &downloads.artifact {
                    let path = libraries_dir.join(self.get_library_path(&library.name));
                    self.file_manager.create_dir_all(path.parent().unwrap()).await?;

                    self.file_manager.download_file(
                        &artifact.url,
                        &path,
                        Some(&artifact.sha1),
                        progress_callback.clone(),
                    ).await?;
                }

                // Download native libraries if needed
                if let (Some(classifiers), Some(natives)) = (&downloads.classifiers, &library.natives) {
                    let os_name = self.get_os_name();
                    if let Some(native_key) = natives.get(os_name) {
                        let native_key = native_key.replace("${arch}", &self.get_arch());

                        if let Some(native_download) = classifiers.get(&native_key) {
                            let path = libraries_dir.join(format!("{:?}-natives-{}", self.get_library_path(&library.name), os_name));
                            self.file_manager.create_dir_all(path.parent().unwrap()).await?;

                            self.file_manager.download_file(
                                &native_download.url,
                                &path,
                                Some(&native_download.sha1),
                                progress_callback.clone(),
                            ).await?;

                            // Extract native libraries if needed
                            if let Some(extract) = &library.extract {
                                let natives_dir = self.config.minecraft_directory.join("versions")
                                    .join(&version_details.id)
                                    .join("natives");

                                self.file_manager.extract_zip(
                                    &path,
                                    &natives_dir,
                                    Some(&extract.exclude),
                                ).await?;
                            }
                        }
                    }
                }
            }
        }

        info!("Successfully downloaded libraries for Minecraft version {}", version_details.id);
        Ok(())
    }

    async fn download_assets(&self, version_details: &VersionDetails, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        info!("Downloading assets for Minecraft version {}", version_details.id);

        let assets_dir = self.config.minecraft_directory.join("assets");
        self.file_manager.create_dir_all(&assets_dir).await?;

        // Download asset index
        let indexes_dir = assets_dir.join("indexes");
        self.file_manager.create_dir_all(&indexes_dir).await?;

        let asset_index_path = indexes_dir.join(format!("{}.json", version_details.assets_index.id));
        self.file_manager.download_file(
            &version_details.assets_index.url,
            &asset_index_path,
            Some(&version_details.assets_index.sha1),
            progress_callback.clone(),
        ).await?;

        // Parse asset index
        let asset_index_content = fs::read_to_string(&asset_index_path).await?;
        let asset_objects: AssetObjects = serde_json::from_str(&asset_index_content)?;

        // Download assets
        let objects_dir = assets_dir.join("objects");
        self.file_manager.create_dir_all(&objects_dir).await?;

        for (asset_name, asset) in &asset_objects.objects {
            let hash_prefix = &asset.hash[0..2];
            let asset_dir = objects_dir.join(hash_prefix);
            self.file_manager.create_dir_all(&asset_dir).await?;

            let asset_path = asset_dir.join(&asset.hash);
            let asset_url = format!("https://resources.download.minecraft.net/{}/{}", hash_prefix, asset.hash);

            self.file_manager.download_file(
                &asset_url,
                &asset_path,
                Some(&asset.hash),
                progress_callback.clone(),
            ).await?;
        }

        info!("Successfully downloaded assets for Minecraft version {}", version_details.id);
        Ok(())
    }

    pub fn launch_game(&self, profile: &Profile, auth_session: &AuthSession) -> Result<()> {
        info!("Launching Minecraft with profile: {}", profile.name);

        // Check if user namespaces are enabled on Linux
        if cfg!(target_os = "linux") && !self.are_user_namespaces_enabled() {
            warn!("User namespaces are not enabled on this system. Minecraft may fail to launch or run with reduced functionality.");
            debug!("Guidance for enabling user namespaces: {}", self.get_user_namespace_guidance());
        }

        // Get version details
        let version_manifest = self.get_version_manifest()?;
        let version_info = version_manifest.versions.iter()
            .find(|v| v.id == profile.version)
            .ok_or_else(|| anyhow!("Version {} not found in manifest", profile.version))?;

        let version_details = self.get_version_details(version_info)?;

        // Build Java command
        let mut command = Command::new(self.get_java_path()?);

        // Add memory arguments
        command.arg(format!("-Xmx{}M", self.config.max_memory));

        // Add JVM arguments
        for arg in &self.config.java_arguments {
            command.arg(arg);
        }

        // Add version-specific JVM arguments
        if let Some(arguments) = &version_details.arguments {
            for arg in &arguments.jvm {
                if let serde_json::Value::String(arg_str) = arg {
                    let arg_str = self.replace_placeholders(&arg_str, profile, auth_session, &version_details)?;
                    command.arg(arg_str);
                }
            }
        }

        // Add main class
        command.arg(&version_details.main_class);

        // Add game arguments
        if let Some(arguments) = &version_details.arguments {
            for arg in &arguments.game {
                if let serde_json::Value::String(arg_str) = arg {
                    let arg_str = self.replace_placeholders(&arg_str, profile, auth_session, &version_details)?;
                    command.arg(arg_str);
                }
            }
        } else if let Some(minecraft_arguments) = &version_details.minecraft_arguments {
            for arg in minecraft_arguments.split(' ') {
                let arg = self.replace_placeholders(arg, profile, auth_session, &version_details)?;
                command.arg(arg);
            }
        }

        // Set working directory
        let game_dir = match &profile.game_directory {
            Some(dir) => dir.clone(),
            None => self.config.minecraft_directory.clone(),
        };
        command.current_dir(game_dir);

        // Launch the process
        info!("Executing command: {:?}", command);

        // Check if sandbox should be disabled from config
        if self.config.disable_sandbox {
            info!("Sandbox mode is disabled in config. Adding --no-sandbox flag.");
            command.arg("--no-sandbox");
        }

        // On Linux, we need to handle potential user namespace issues
        let child = if cfg!(target_os = "linux") {
            // Try to launch with current settings
            match command.stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
            {
                Ok(child) => {
                    info!("Minecraft launched with PID: {}", child.id());
                    child
                },
                Err(e) => {
                    // Check if the error is related to user namespaces and we haven't already disabled sandbox
                    if (e.to_string().contains("CanCreateUserNamespace") || e.to_string().contains("EPERM")) && !self.config.disable_sandbox {
                        warn!("Failed to launch with user namespaces: {}. Trying with --no-sandbox flag.", e);

                        // Add --no-sandbox flag to disable user namespaces
                        command.arg("--no-sandbox");

                        // Try again with the no-sandbox flag
                        match command.spawn() {
                            Ok(child) => {
                                info!("Minecraft launched with PID: {} (no-sandbox mode)", child.id());
                                info!("Consider setting 'disable_sandbox' to true in the launcher settings to avoid this issue in the future.");
                                child
                            },
                            Err(e2) => {
                                error!("Failed to launch even with --no-sandbox flag: {}", e2);

                                // Check if user namespaces are enabled and provide guidance
                                let user_ns_enabled = self.are_user_namespaces_enabled();
                                let guidance = if !user_ns_enabled {
                                    format!("\n\nUser namespaces are not enabled on your system. {}", self.get_user_namespace_guidance())
                                } else {
                                    String::new()
                                };

                                return Err(anyhow!("Failed to launch Minecraft: {}. Tried with --no-sandbox but still failed: {}.{}", 
                                    e, e2, guidance));
                            }
                        }
                    } else {
                        // If it's not a user namespace issue or sandbox is already disabled, just return the original error
                        error!("Failed to launch Minecraft: {}", e);
                        return Err(anyhow!("Failed to launch Minecraft: {}", e));
                    }
                }
            }
        } else {
            // On non-Linux platforms, just spawn normally
            command.stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?
        };

        info!("Minecraft launched with PID: {}", child.id());
        Ok(())
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
        let mut allow = false;

        for rule in rules {
            let matches = match &rule.os {
                Some(os) => {
                    let os_name_matches = match &os.name {
                        Some(name) => self.get_os_name() == name,
                        None => true,
                    };

                    let os_version_matches = match &os.version {
                        Some(version) => {
                            // This would need a proper regex match in a real implementation
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

        let game_dir = match &profile.game_directory {
            Some(dir) => dir.clone(),
            None => self.config.minecraft_directory.clone(),
        };

        let arg = arg.replace("${auth_player_name}", &minecraft_profile.name)
            .replace("${version_name}", &version_details.id)
            .replace("${game_directory}", &game_dir.to_string_lossy())
            .replace("${assets_root}", &self.config.minecraft_directory.join("assets").to_string_lossy())
            .replace("${assets_index_name}", &version_details.assets_index.id)
            .replace("${auth_uuid}", &minecraft_profile.id)
            .replace("${auth_access_token}", &auth_session.minecraft_token.clone().unwrap_or_default())
            .replace("${user_type}", "msa")
            .replace("${version_type}", &version_details.r#type);

        Ok(arg)
    }
}
