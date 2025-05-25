// Version management for Minecraft

use anyhow::{Result, anyhow};
use log::{info, warn, error, debug};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use sha1::{Sha1, Digest};

use crate::file_manager::{FileManager, DownloadProgress};
use super::models::{VersionManifest, VersionDetails, AssetObjects, AssetObject, Library, Rule, Os};

// Minecraft version manifest URL
const VERSION_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Fetches the Minecraft version manifest
pub async fn fetch_version_manifest(file_manager: &FileManager) -> Result<VersionManifest> {
    info!("Fetching Minecraft version manifest");

    // Download the version manifest
    let manifest_json = file_manager.download_string(VERSION_MANIFEST_URL).await?;

    // Parse the manifest
    let manifest: VersionManifest = serde_json::from_str(&manifest_json)?;

    Ok(manifest)
}

/// Fetches the details for a specific Minecraft version
pub async fn fetch_version_details(file_manager: &FileManager, version_id: &str, manifest: &VersionManifest) -> Result<VersionDetails> {
    info!("Fetching details for Minecraft version {}", version_id);

    // Find the version in the manifest
    let version_info = manifest.versions.iter()
        .find(|v| v.id == version_id)
        .ok_or_else(|| anyhow!("Version {} not found in manifest", version_id))?;

    // Download the version details
    let details_json = file_manager.download_string(&version_info.url).await?;

    // Parse the details
    let details: VersionDetails = serde_json::from_str(&details_json)?;

    Ok(details)
}

/// Downloads a Minecraft version
pub async fn download_version(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
) -> Result<()> {
    info!("Downloading Minecraft version {}", version_details.id);

    // Create the version directory
    let version_dir = minecraft_dir.join("versions").join(&version_details.id);
    file_manager.create_dir_all(&version_dir).await?;

    // Download the client jar
    let client_download = version_details.downloads.get("client")
        .ok_or_else(|| anyhow!("Client download not found for version {}", version_details.id))?;

    let client_jar_path = version_dir.join(format!("{}.jar", version_details.id));

    info!("Downloading client jar for version {}", version_details.id);
    file_manager.download_file(
        &client_download.url,
        &client_jar_path,
        Some(&client_download.sha1),
        progress_callback.clone(),
    ).await?;

    // Download the libraries
    download_libraries(file_manager, minecraft_dir, version_details, progress_callback.clone()).await?;

    // Download the assets
    download_assets(file_manager, minecraft_dir, version_details, progress_callback).await?;

    // Save the version json
    let version_json_path = version_dir.join(format!("{}.json", version_details.id));
    let version_json = serde_json::to_string_pretty(version_details)?;
    file_manager.write_to_file(&version_json_path, &version_json).await?;

    info!("Minecraft version {} downloaded successfully", version_details.id);
    Ok(())
}

/// Downloads the libraries for a Minecraft version
async fn download_libraries(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
) -> Result<()> {
    info!("Downloading libraries for Minecraft version {}", version_details.id);

    let libraries_dir = minecraft_dir.join("libraries");
    file_manager.create_dir_all(&libraries_dir).await?;

    let os_name = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let os_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    };

    // Download each library
    for library in &version_details.libraries {
        // Check if the library should be downloaded for this OS
        if !should_download_library(library, os_name, os_arch) {
            continue;
        }

        // Download the main artifact if it exists
        if let Some(downloads) = &library.downloads {
            if let Some(artifact) = &downloads.artifact {
                // Determine the library path from the library name
                // Format: group:artifact:version
                // Example: org.lwjgl:lwjgl:3.2.2
                let parts: Vec<&str> = library.name.split(':').collect();
                if parts.len() < 3 {
                    warn!("Invalid library name format: {}", library.name);
                    continue;
                }

                let group = parts[0].replace('.', "/");
                let artifact_id = parts[1];
                let version = parts[2];

                let library_path = libraries_dir.join(format!("{}/{}/{}/{}-{}.jar", 
                    group, artifact_id, version, artifact_id, version));
                let library_dir = library_path.parent().unwrap();

                file_manager.create_dir_all(library_dir).await?;

                info!("Downloading library: {}", library.name);
                file_manager.download_file(
                    &artifact.url,
                    &library_path,
                    Some(&artifact.sha1),
                    progress_callback.clone(),
                ).await?;
            }

            // Download natives if they exist
            if let Some(classifiers) = &downloads.classifiers {
                if let Some(natives) = &library.natives {
                    if let Some(native_key) = natives.get(os_name) {
                        if let Some(native_artifact) = classifiers.get(native_key) {
                            // Determine the native library path from the library name
                            // Format: group:artifact:version
                            // Example: org.lwjgl:lwjgl:3.2.2
                            let parts: Vec<&str> = library.name.split(':').collect();
                            if parts.len() < 3 {
                                warn!("Invalid library name format for native: {}", library.name);
                                continue;
                            }

                            let group = parts[0].replace('.', "/");
                            let artifact_id = parts[1];
                            let version = parts[2];

                            // For natives, we need to add the classifier to the filename
                            let native_path = libraries_dir.join(format!("{}/{}/{}/{}-{}-{}.jar", 
                                group, artifact_id, version, artifact_id, version, native_key));
                            let native_dir = native_path.parent().unwrap();

                            file_manager.create_dir_all(native_dir).await?;

                            info!("Downloading native library: {}", library.name);
                            file_manager.download_file(
                                &native_artifact.url,
                                &native_path,
                                Some(&native_artifact.sha1),
                                progress_callback.clone(),
                            ).await?;

                            // Extract natives if needed
                            if let Some(extract) = &library.extract {
                                let natives_dir = minecraft_dir.join("versions")
                                    .join(&version_details.id)
                                    .join("natives");

                                file_manager.create_dir_all(&natives_dir).await?;

                                info!("Extracting native library: {}", library.name);
                                extract_natives(
                                    &native_path,
                                    &natives_dir,
                                    &extract.exclude,
                                )?;
                            }
                        }
                    }
                }
            }
        }
    }

    info!("Libraries downloaded successfully");
    Ok(())
}

/// Checks if a library should be downloaded for the current OS
fn should_download_library(library: &Library, os_name: &str, os_arch: &str) -> bool {
    // If there are no rules, the library is always included
    if library.rules.is_none() {
        return true;
    }

    let rules = library.rules.as_ref().unwrap();
    let mut allowed = false;

    for rule in rules {
        let action_allowed = rule.action == "allow";

        // If there's no OS specified, the rule applies to all OSes
        if rule.os.is_none() {
            allowed = action_allowed;
            continue;
        }

        let os = rule.os.as_ref().unwrap();

        // Check if the rule applies to the current OS
        let os_matches = match &os.name {
            Some(name) => name == os_name,
            None => true,
        };

        // Check if the rule applies to the current OS version
        let version_matches = match &os.version {
            Some(_version) => {
                // TODO: Implement version matching if needed
                true
            },
            None => true,
        };

        // Check if the rule applies to the current OS architecture
        let arch_matches = match &os.arch {
            Some(arch) => arch == os_arch,
            None => true,
        };

        // If all conditions match, apply the rule
        if os_matches && version_matches && arch_matches {
            allowed = action_allowed;
        }
    }

    allowed
}

/// Extracts native libraries from a JAR file
fn extract_natives(jar_path: &Path, output_dir: &Path, exclude: &[String]) -> Result<()> {
    let file = std::fs::File::open(jar_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_name = file.name().to_string();

        // Skip excluded files
        if exclude.iter().any(|pattern| file_name.contains(pattern)) {
            continue;
        }

        // Skip directories
        if file.is_dir() {
            continue;
        }

        let output_path = output_dir.join(file_name);

        // Create parent directories if they don't exist
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut output_file = std::fs::File::create(&output_path)?;
        std::io::copy(&mut file, &mut output_file)?;
    }

    Ok(())
}

/// Downloads the assets for a Minecraft version
async fn download_assets(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
) -> Result<()> {
    info!("Downloading assets for Minecraft version {}", version_details.id);

    let assets_dir = minecraft_dir.join("assets");
    file_manager.create_dir_all(&assets_dir).await?;

    let indexes_dir = assets_dir.join("indexes");
    file_manager.create_dir_all(&indexes_dir).await?;

    let objects_dir = assets_dir.join("objects");
    file_manager.create_dir_all(&objects_dir).await?;

    // Get the asset index
    let asset_index = version_details.assets_index.as_ref()
        .ok_or_else(|| anyhow!("Asset index not found for version {}", version_details.id))?;

    // Download the asset index
    let index_path = indexes_dir.join(format!("{}.json", asset_index.id));

    info!("Downloading asset index for version {}", version_details.id);
    file_manager.download_file(
        &asset_index.url,
        &index_path,
        Some(&asset_index.sha1),
        progress_callback.clone(),
    ).await?;

    // Parse the asset index
    let index_json = file_manager.read_to_string(&index_path).await?;
    let asset_objects: AssetObjects = serde_json::from_str(&index_json)?;

    // Download each asset
    for (asset_name, asset) in &asset_objects.objects {
        let hash_prefix = &asset.hash[0..2];
        let hash_dir = objects_dir.join(hash_prefix);
        file_manager.create_dir_all(&hash_dir).await?;

        let asset_path = hash_dir.join(&asset.hash);

        // Skip if the asset already exists and has the correct hash
        if asset_path.exists() {
            if verify_file_hash(&asset_path, &asset.hash)? {
                continue;
            }
        }

        let asset_url = format!(
            "https://resources.download.minecraft.net/{}/{}",
            hash_prefix, asset.hash
        );

        info!("Downloading asset: {}", asset_name);
        file_manager.download_file(
            &asset_url,
            &asset_path,
            Some(&asset.hash),
            progress_callback.clone(),
        ).await?;
    }

    info!("Assets downloaded successfully");
    Ok(())
}

/// Verifies the hash of a file
fn verify_file_hash(file_path: &Path, expected_hash: &str) -> Result<bool> {
    let mut file = std::fs::File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let mut hasher = Sha1::new();
    hasher.update(&buffer);
    let hash = format!("{:x}", hasher.finalize());

    Ok(hash == expected_hash)
}

/// Checks if a Minecraft version is installed
pub fn is_version_installed(minecraft_dir: &Path, version_id: &str) -> bool {
    let version_dir = minecraft_dir.join("versions").join(version_id);
    let jar_path = version_dir.join(format!("{}.jar", version_id));
    let json_path = version_dir.join(format!("{}.json", version_id));

    jar_path.exists() && json_path.exists()
}

/// Gets the list of installed Minecraft versions
pub fn get_installed_versions(minecraft_dir: &Path) -> Vec<String> {
    let versions_dir = minecraft_dir.join("versions");

    if !versions_dir.exists() {
        return Vec::new();
    }

    let entries = match fs::read_dir(versions_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut versions = Vec::new();

    for entry in entries {
        if let Ok(entry) = entry {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    let jar_path = entry.path().join(format!("{}.jar", name));
                    let json_path = entry.path().join(format!("{}.json", name));

                    if jar_path.exists() && json_path.exists() {
                        versions.push(name.to_string());
                    }
                }
            }
        }
    }

    versions
}

/// Loads the details for an installed Minecraft version
pub async fn load_version_details(file_manager: &FileManager, minecraft_dir: &Path, version_id: &str) -> Result<VersionDetails> {
    let json_path = minecraft_dir.join("versions").join(version_id).join(format!("{}.json", version_id));

    if !json_path.exists() {
        return Err(anyhow!("Version {} is not installed", version_id));
    }

    let json = file_manager.read_to_string(&json_path).await?;
    let details: VersionDetails = serde_json::from_str(&json)?;

    Ok(details)
}

/// Builds the classpath for a Minecraft version
pub fn build_classpath(minecraft_dir: &Path, version_details: &VersionDetails) -> Result<String> {
    let mut classpath = String::new();

    // Add the client jar
    let client_jar = minecraft_dir.join("versions")
        .join(&version_details.id)
        .join(format!("{}.jar", version_details.id));

    classpath.push_str(&client_jar.to_string_lossy());

    // Get the OS name for natives
    let os_name = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let os_arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "unknown"
    };

    // Add the libraries
    for library in &version_details.libraries {
        // Check if the library should be included for this OS
        if !should_download_library(library, os_name, os_arch) {
            continue;
        }

        if let Some(downloads) = &library.downloads {
            if let Some(artifact) = &downloads.artifact {
                // Determine the library path from the library name
                // Format: group:artifact:version
                // Example: org.lwjgl:lwjgl:3.2.2
                let parts: Vec<&str> = library.name.split(':').collect();
                if parts.len() < 3 {
                    warn!("Invalid library name format: {}", library.name);
                    continue;
                }

                let group = parts[0].replace('.', "/");
                let artifact_id = parts[1];
                let version = parts[2];

                let library_path = minecraft_dir.join("libraries").join(format!("{}/{}/{}/{}-{}.jar", 
                    group, artifact_id, version, artifact_id, version));

                if library_path.exists() {
                    classpath.push(if cfg!(windows) { ';' } else { ':' });
                    classpath.push_str(&library_path.to_string_lossy());
                }
            }
        }
    }

    Ok(classpath)
}
