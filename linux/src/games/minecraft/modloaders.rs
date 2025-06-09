// Modloader implementations for Minecraft

use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::Command;
use log::{info, warn, error, debug};
use std::fs;

use crate::config::{Profile, ModLoader};
use crate::file_manager::{FileManager, DownloadProgress};

use super::models::VersionDetails;

/// Fetch available Forge versions for a given Minecraft version
pub fn fetch_forge_versions(minecraft_version: &str) -> Result<Vec<String>> {
    info!("Fetching Forge versions for Minecraft {}", minecraft_version);

    // For Forge, we'll use a combination of hardcoded mappings and a format pattern
    // This is a simplified approach since we can't easily parse the Maven metadata

    // Known mappings for specific Minecraft versions
    let known_versions = match minecraft_version {
        "1.21.5" => vec!["1.21.5-47.1.0".to_string()],
        "1.21.1" => vec!["1.21.1-47.0.1".to_string()],
        "1.20.4" => vec!["1.20.4-49.0.3".to_string(), "1.20.4-49.0.2".to_string(), "1.20.4-49.0.1".to_string()],
        "1.20.1" => vec!["1.20.1-47.2.0".to_string(), "1.20.1-47.1.0".to_string(), "1.20.1-47.0.0".to_string()],
        "1.19.4" => vec!["1.19.4-45.1.0".to_string(), "1.19.4-45.0.0".to_string()],
        "1.19.2" => vec!["1.19.2-43.2.0".to_string(), "1.19.2-43.1.0".to_string(), "1.19.2-43.0.0".to_string()],
        "1.18.2" => vec!["1.18.2-40.2.0".to_string(), "1.18.2-40.1.0".to_string(), "1.18.2-40.0.0".to_string()],
        "1.17.1" => vec!["1.17.1-37.1.1".to_string(), "1.17.1-37.1.0".to_string(), "1.17.1-37.0.0".to_string()],
        "1.16.5" => vec!["1.16.5-36.2.39".to_string(), "1.16.5-36.2.0".to_string(), "1.16.5-36.1.0".to_string()],
        _ => {
            // For unknown versions, return an empty vector
            // The user will need to select a specific version
            warn!("No known Forge versions for Minecraft {}", minecraft_version);
            vec![]
        }
    };

    // If we have known versions, use the first one as the "latest"
    let mut versions = if !known_versions.is_empty() {
        // Use the first known version as the default/latest
        vec![known_versions[0].clone()]
    } else {
        vec![]
    };

    // Add the known versions
    versions.extend(known_versions);

    // Remove duplicates
    versions.sort();
    versions.dedup();

    Ok(versions)
}

/// Fetch available Fabric versions for a given Minecraft version
pub fn fetch_fabric_versions(minecraft_version: &str) -> Result<Vec<String>> {
    info!("Fetching Fabric versions for Minecraft {}", minecraft_version);

    // For Fabric, we'll use a set of common loader versions
    // This is a simplified approach since we can't easily fetch from the API

    let common_versions = vec![
        "0.15.3", "0.15.2", "0.15.1", "0.15.0",
        "0.14.21", "0.14.20", "0.14.19", "0.14.18",
        "0.14.17", "0.14.16", "0.14.15", "0.14.14",
    ];

    let mut versions = Vec::new();
    for version in common_versions {
        versions.push(version.to_string());
    }

    // Add a "latest" option
    versions.push("latest".to_string());

    Ok(versions)
}

/// Fetch available Quilt versions for a given Minecraft version
pub fn fetch_quilt_versions(minecraft_version: &str) -> Result<Vec<String>> {
    info!("Fetching Quilt versions for Minecraft {}", minecraft_version);

    // For Quilt, we'll use a set of common loader versions
    // This is a simplified approach since we can't easily fetch from the API

    let common_versions = vec![
        "0.20.2", "0.20.1", "0.20.0",
        "0.19.2", "0.19.1", "0.19.0",
        "0.18.10", "0.18.9", "0.18.8",
    ];

    let mut versions = Vec::new();
    for version in common_versions {
        versions.push(version.to_string());
    }

    // Add a "latest" option
    versions.push("latest".to_string());

    Ok(versions)
}

/// Fetch available NeoForge versions for a given Minecraft version
pub fn fetch_neoforge_versions(minecraft_version: &str) -> Result<Vec<String>> {
    info!("Fetching NeoForge versions for Minecraft {}", minecraft_version);

    // For NeoForge, we'll use a combination of hardcoded mappings and a format pattern
    // This is a simplified approach since we can't easily parse the Maven metadata

    // Known mappings for specific Minecraft versions
    let known_versions = match minecraft_version {
        "1.21.1" => vec!["1.21.1-47.0.1".to_string()],
        "1.20.4" => vec!["1.20.4-49.0.3".to_string(), "1.20.4-49.0.2".to_string(), "1.20.4-49.0.1".to_string()],
        "1.20.1" => vec!["1.20.1-47.2.0".to_string(), "1.20.1-47.1.0".to_string(), "1.20.1-47.0.0".to_string()],
        "1.19.4" => vec!["1.19.4-45.1.0".to_string(), "1.19.4-45.0.0".to_string()],
        "1.19.2" => vec!["1.19.2-43.2.0".to_string(), "1.19.2-43.1.0".to_string(), "1.19.2-43.0.0".to_string()],
        _ => {
            // For unknown versions, return an empty vector
            // The user will need to select a specific version
            warn!("No known NeoForge versions for Minecraft {}", minecraft_version);
            vec![]
        }
    };

    // If we have known versions, use the first one as the "latest"
    let mut versions = if !known_versions.is_empty() {
        // Use the first known version as the default/latest
        vec![known_versions[0].clone()]
    } else {
        vec![]
    };

    // Add the known versions
    versions.extend(known_versions);

    // Remove duplicates
    versions.sort();
    versions.dedup();

    Ok(versions)
}

/// Install Forge modloader
pub async fn install_forge(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    profile: &Profile,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
    java_path: &Path,
) -> Result<()> {
    info!("Installing Forge for Minecraft version {}", version_details.id);

    // Get the Forge version from the profile or use a recommended version for this Minecraft version
    let forge_version = if let Some(version) = &profile.mod_loader_version {
        version.clone()
    } else {
        // For newer Minecraft versions, we need to look up the recommended version
        // For now, use a specific version that's known to work with this Minecraft version
        let mc_version = &version_details.id;

        // Map Minecraft versions to known working Forge versions
        // These are examples and should be updated with actual compatible versions
        match mc_version.as_str() {
            "1.21.5" => format!("1.21.5-47.1.0"), // Example version, update with actual version
            "1.21.1" => format!("1.21.1-47.0.1"), // Example version, update with actual version
            "1.20.4" => format!("1.20.4-49.0.3"),
            "1.20.1" => format!("1.20.1-47.2.0"),
            "1.19.4" => format!("1.19.4-45.1.0"),
            "1.19.2" => format!("1.19.2-43.2.0"),
            "1.18.2" => format!("1.18.2-40.2.0"),
            "1.17.1" => format!("1.17.1-37.1.1"),
            "1.16.5" => format!("1.16.5-36.2.39"),
            _ => {
                // For unknown versions, we need to provide a more helpful error message
                warn!("No known Forge version for Minecraft {}", mc_version);
                return Err(anyhow!(
                    "No known Forge version for Minecraft {}. \
                    Please select a specific Forge version in the profile settings. \
                    You can find compatible Forge versions at https://files.minecraftforge.net/net/minecraftforge/forge/",
                    mc_version
                ));
            }
        }
    };

    // Forge installer URL
    let forge_installer_url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{}/forge-{}-installer.jar",
        forge_version, forge_version
    );

    // Download the Forge installer
    let forge_dir = minecraft_dir.join("forge");
    file_manager.create_dir_all(&forge_dir).await?;

    let installer_path = forge_dir.join(format!("forge-{}-installer.jar", forge_version));

    // Maximum number of retry attempts
    const MAX_RETRIES: usize = 3;
    let mut retry_count = 0;
    let mut last_error = None;

    // Try to download and verify the installer
    while retry_count < MAX_RETRIES {
        if retry_count > 0 {
            info!("Retry attempt {} of {} for downloading Forge installer", retry_count, MAX_RETRIES);
            // Wait a bit before retrying
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Remove the file if it exists from a previous attempt
            if installer_path.exists() {
                if let Err(e) = fs::remove_file(&installer_path) {
                    warn!("Failed to remove existing installer file: {}", e);
                }
            }
        }

        // Download the installer
        info!("Downloading Forge installer from {}", forge_installer_url);
        match file_manager.download_file(
            &forge_installer_url,
            &installer_path,
            None, // No hash verification for now
            progress_callback.clone(),
        ).await {
            Ok(()) => {
                // Verify the downloaded file is a valid JAR file
                info!("Verifying Forge installer JAR file");
                match verify_jar_file(&installer_path).await {
                    Ok(true) => {
                        // File is valid, break out of the retry loop
                        break;
                    },
                    Ok(false) => {
                        // File is invalid, try again
                        warn!("Downloaded Forge installer is not a valid JAR file. Retrying...");
                        last_error = Some(anyhow!("Downloaded Forge installer is not a valid JAR file"));
                        retry_count += 1;
                    },
                    Err(e) => {
                        // Error during verification, try again
                        warn!("Error verifying Forge installer: {}. Retrying...", e);
                        last_error = Some(e);
                        retry_count += 1;
                    }
                }
            },
            Err(e) => {
                // Error during download, try again
                warn!("Error downloading Forge installer: {}. Retrying...", e);
                last_error = Some(e);
                retry_count += 1;
            }
        }
    }

    // Check if we've exhausted all retry attempts
    if retry_count >= MAX_RETRIES {
        let error_msg = last_error.unwrap_or_else(|| anyhow!("Failed to download and verify Forge installer after {} attempts", MAX_RETRIES));

        // Provide a more helpful error message with troubleshooting steps
        return Err(anyhow!(
            "Failed to download and verify Forge installer: {}. \
            This could be due to network issues, server problems, or an incorrect version number. \
            Please try the following:\n\
            1. Check your internet connection\n\
            2. Try a different Forge version\n\
            3. Verify that the Minecraft version {} is compatible with Forge\n\
            4. Try again later as the server might be temporarily unavailable",
            error_msg, version_details.id
        ));
    }

    // Run the installer
    info!("Running Forge installer");
    let status = Command::new(java_path)
        .current_dir(minecraft_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .status()?;

    if !status.success() {
        return Err(anyhow!("Forge installation failed with exit code: {}", status));
    }

    info!("Forge installation completed successfully");
    Ok(())
}

/// Install Fabric modloader
pub async fn install_fabric(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    profile: &Profile,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
    java_path: &Path,
) -> Result<()> {
    info!("Installing Fabric for Minecraft version {}", version_details.id);

    // Fabric installer URL
    let fabric_installer_url = "https://maven.fabricmc.net/net/fabricmc/fabric-installer/0.11.2/fabric-installer-0.11.2.jar";

    // Download the Fabric installer
    let fabric_dir = minecraft_dir.join("fabric");
    file_manager.create_dir_all(&fabric_dir).await?;

    // Create a minimal launcher_profiles.json file if it doesn't exist
    // This is needed because the Fabric installer looks for this file
    let launcher_profiles_path = minecraft_dir.join("launcher_profiles.json");
    if !launcher_profiles_path.exists() {
        info!("Creating minimal launcher_profiles.json file");
        let minimal_profiles = r#"{"profiles": {}}"#;
        file_manager.write_to_file(&launcher_profiles_path, minimal_profiles).await?;
    }

    let installer_path = fabric_dir.join("fabric-installer.jar");

    // Maximum number of retry attempts
    const MAX_RETRIES: usize = 3;
    let mut retry_count = 0;
    let mut last_error = None;

    // Try to download and verify the installer
    while retry_count < MAX_RETRIES {
        if retry_count > 0 {
            info!("Retry attempt {} of {} for downloading Fabric installer", retry_count, MAX_RETRIES);
            // Wait a bit before retrying
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Remove the file if it exists from a previous attempt
            if installer_path.exists() {
                if let Err(e) = fs::remove_file(&installer_path) {
                    warn!("Failed to remove existing installer file: {}", e);
                }
            }
        }

        // Download the installer
        info!("Downloading Fabric installer from {}", fabric_installer_url);
        match file_manager.download_file(
            &fabric_installer_url,
            &installer_path,
            None, // No hash verification for now
            progress_callback.clone(),
        ).await {
            Ok(()) => {
                // Verify the downloaded file is a valid JAR file
                info!("Verifying Fabric installer JAR file");
                match verify_jar_file(&installer_path).await {
                    Ok(true) => {
                        // File is valid, break out of the retry loop
                        break;
                    },
                    Ok(false) => {
                        // File is invalid, try again
                        warn!("Downloaded Fabric installer is not a valid JAR file. Retrying...");
                        last_error = Some(anyhow!("Downloaded Fabric installer is not a valid JAR file"));
                        retry_count += 1;
                    },
                    Err(e) => {
                        // Error during verification, try again
                        warn!("Error verifying Fabric installer: {}. Retrying...", e);
                        last_error = Some(e);
                        retry_count += 1;
                    }
                }
            },
            Err(e) => {
                // Error during download, try again
                warn!("Error downloading Fabric installer: {}. Retrying...", e);
                last_error = Some(e);
                retry_count += 1;
            }
        }
    }

    // Check if we've exhausted all retry attempts
    if retry_count >= MAX_RETRIES {
        let error_msg = last_error.unwrap_or_else(|| anyhow!("Failed to download and verify Fabric installer after {} attempts", MAX_RETRIES));

        // Provide a more helpful error message with troubleshooting steps
        return Err(anyhow!(
            "Failed to download and verify Fabric installer: {}. \
            This could be due to network issues, server problems, or an incorrect version number. \
            Please try the following:\n\
            1. Check your internet connection\n\
            2. Verify that the Minecraft version {} is compatible with Fabric\n\
            3. Try again later as the server might be temporarily unavailable\n\
            4. Check if a newer version of the Fabric installer is available",
            error_msg, version_details.id
        ));
    }

    // Run the installer
    info!("Running Fabric installer");

    // Run the installer with verbose output to help diagnose issues
    let status = Command::new(java_path)
        .current_dir(minecraft_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("client")
        .arg("-mcversion")
        .arg(&version_details.id)
        .arg("-dir")
        .arg(minecraft_dir)
        .arg("-noprofile") // Don't create a launcher profile
        .arg("-verbose") // Enable verbose output
        .status()?;

    if !status.success() {
        return Err(anyhow!("Fabric installation failed with exit code: {}", status));
    }

    info!("Fabric installation completed successfully");
    Ok(())
}

/// Install Quilt modloader
pub async fn install_quilt(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    profile: &Profile,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
    java_path: &Path,
) -> Result<()> {
    info!("Installing Quilt for Minecraft version {}", version_details.id);

    // Quilt installer URL - use a specific version instead of "latest"
    let quilt_installer_url = "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/0.8.1/quilt-installer-0.8.1.jar";

    // Download the Quilt installer
    let quilt_dir = minecraft_dir.join("quilt");
    file_manager.create_dir_all(&quilt_dir).await?;

    let installer_path = quilt_dir.join("quilt-installer.jar");

    // Maximum number of retry attempts
    const MAX_RETRIES: usize = 3;
    let mut retry_count = 0;
    let mut last_error = None;

    // Try to download and verify the installer
    while retry_count < MAX_RETRIES {
        if retry_count > 0 {
            info!("Retry attempt {} of {} for downloading Quilt installer", retry_count, MAX_RETRIES);
            // Wait a bit before retrying
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Remove the file if it exists from a previous attempt
            if installer_path.exists() {
                if let Err(e) = fs::remove_file(&installer_path) {
                    warn!("Failed to remove existing installer file: {}", e);
                }
            }
        }

        // Download the installer
        info!("Downloading Quilt installer from {}", quilt_installer_url);
        match file_manager.download_file(
            &quilt_installer_url,
            &installer_path,
            None, // No hash verification for now
            progress_callback.clone(),
        ).await {
            Ok(()) => {
                // Verify the downloaded file is a valid JAR file
                info!("Verifying Quilt installer JAR file");
                match verify_jar_file(&installer_path).await {
                    Ok(true) => {
                        // File is valid, break out of the retry loop
                        break;
                    },
                    Ok(false) => {
                        // File is invalid, try again
                        warn!("Downloaded Quilt installer is not a valid JAR file. Retrying...");
                        last_error = Some(anyhow!("Downloaded Quilt installer is not a valid JAR file"));
                        retry_count += 1;
                    },
                    Err(e) => {
                        // Error during verification, try again
                        warn!("Error verifying Quilt installer: {}. Retrying...", e);
                        last_error = Some(e);
                        retry_count += 1;
                    }
                }
            },
            Err(e) => {
                // Error during download, try again
                warn!("Error downloading Quilt installer: {}. Retrying...", e);
                last_error = Some(e);
                retry_count += 1;
            }
        }
    }

    // Check if we've exhausted all retry attempts
    if retry_count >= MAX_RETRIES {
        let error_msg = last_error.unwrap_or_else(|| anyhow!("Failed to download and verify Quilt installer after {} attempts", MAX_RETRIES));

        // Provide a more helpful error message with troubleshooting steps
        return Err(anyhow!(
            "Failed to download and verify Quilt installer: {}. \
            This could be due to network issues, server problems, or an incorrect version number. \
            Please try the following:\n\
            1. Check your internet connection\n\
            2. Verify that the Minecraft version {} is compatible with Quilt\n\
            3. Try again later as the server might be temporarily unavailable\n\
            4. Check if Quilt supports this Minecraft version (Quilt is newer than Fabric and may not support all versions)\n\
            5. Consider using Fabric instead if Quilt is not available for this version",
            error_msg, version_details.id
        ));
    }

    // Run the installer
    info!("Running Quilt installer");
    let status = Command::new(java_path)
        .current_dir(minecraft_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("install")
        .arg("client")
        .arg(&version_details.id)
        .arg("--install-dir")
        .arg(minecraft_dir)
        .status()?;

    if !status.success() {
        return Err(anyhow!("Quilt installation failed with exit code: {}", status));
    }

    info!("Quilt installation completed successfully");
    Ok(())
}

/// Install NeoForge modloader
pub async fn install_neoforge(
    file_manager: &FileManager,
    minecraft_dir: &Path,
    profile: &Profile,
    version_details: &VersionDetails,
    progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone,
    java_path: &Path,
) -> Result<()> {
    info!("Installing NeoForge for Minecraft version {}", version_details.id);

    // Get the NeoForge version from the profile or use a recommended version for this Minecraft version
    let neoforge_version = if let Some(version) = &profile.mod_loader_version {
        version.clone()
    } else {
        // For newer Minecraft versions, we need to look up the recommended version
        // For now, use a specific version that's known to work with this Minecraft version
        let mc_version = &version_details.id;

        // Map Minecraft versions to known working NeoForge versions
        // These are examples and should be updated with actual compatible versions
        match mc_version.as_str() {
            "1.21.1" => format!("1.21.1-47.0.1"), // Example version, update with actual version
            "1.20.4" => format!("1.20.4-49.0.3"),
            "1.20.1" => format!("1.20.1-47.2.0"),
            "1.19.4" => format!("1.19.4-45.1.0"),
            "1.19.2" => format!("1.19.2-43.2.0"),
            _ => {
                // For unknown versions, we need to provide a more helpful error message
                warn!("No known NeoForge version for Minecraft {}", mc_version);
                return Err(anyhow!(
                    "No known NeoForge version for Minecraft {}. \
                    Please select a specific NeoForge version in the profile settings. \
                    You can find compatible NeoForge versions at https://neoforged.net/",
                    mc_version
                ));
            }
        }
    };

    // NeoForge installer URL
    let neoforge_installer_url = format!(
        "https://maven.neoforged.net/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
        neoforge_version, neoforge_version
    );

    // Download the NeoForge installer
    let neoforge_dir = minecraft_dir.join("neoforge");
    file_manager.create_dir_all(&neoforge_dir).await?;

    let installer_path = neoforge_dir.join(format!("neoforge-{}-installer.jar", neoforge_version));

    // Maximum number of retry attempts
    const MAX_RETRIES: usize = 3;
    let mut retry_count = 0;
    let mut last_error = None;

    // Try to download and verify the installer
    while retry_count < MAX_RETRIES {
        if retry_count > 0 {
            info!("Retry attempt {} of {} for downloading NeoForge installer", retry_count, MAX_RETRIES);
            // Wait a bit before retrying
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

            // Remove the file if it exists from a previous attempt
            if installer_path.exists() {
                if let Err(e) = fs::remove_file(&installer_path) {
                    warn!("Failed to remove existing installer file: {}", e);
                }
            }
        }

        // Download the installer
        info!("Downloading NeoForge installer from {}", neoforge_installer_url);
        match file_manager.download_file(
            &neoforge_installer_url,
            &installer_path,
            None, // No hash verification for now
            progress_callback.clone(),
        ).await {
            Ok(()) => {
                // Verify the downloaded file is a valid JAR file
                info!("Verifying NeoForge installer JAR file");
                match verify_jar_file(&installer_path).await {
                    Ok(true) => {
                        // File is valid, break out of the retry loop
                        break;
                    },
                    Ok(false) => {
                        // File is invalid, try again
                        warn!("Downloaded NeoForge installer is not a valid JAR file. Retrying...");
                        last_error = Some(anyhow!("Downloaded NeoForge installer is not a valid JAR file"));
                        retry_count += 1;
                    },
                    Err(e) => {
                        // Error during verification, try again
                        warn!("Error verifying NeoForge installer: {}. Retrying...", e);
                        last_error = Some(e);
                        retry_count += 1;
                    }
                }
            },
            Err(e) => {
                // Error during download, try again
                warn!("Error downloading NeoForge installer: {}. Retrying...", e);
                last_error = Some(e);
                retry_count += 1;
            }
        }
    }

    // Check if we've exhausted all retry attempts
    if retry_count >= MAX_RETRIES {
        let error_msg = last_error.unwrap_or_else(|| anyhow!("Failed to download and verify NeoForge installer after {} attempts", MAX_RETRIES));

        // Provide a more helpful error message with troubleshooting steps
        return Err(anyhow!(
            "Failed to download and verify NeoForge installer: {}. \
            This could be due to network issues, server problems, or an incorrect version number. \
            Please try the following:\n\
            1. Check your internet connection\n\
            2. Try a different NeoForge version\n\
            3. Verify that the Minecraft version {} is compatible with NeoForge\n\
            4. Try again later as the server might be temporarily unavailable\n\
            5. Check if NeoForge is available for this Minecraft version (NeoForge is newer than Forge and may not support all versions)",
            error_msg, version_details.id
        ));
    }

    // Run the installer
    info!("Running NeoForge installer");
    let status = Command::new(java_path)
        .current_dir(minecraft_dir)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .status()?;

    if !status.success() {
        return Err(anyhow!("NeoForge installation failed with exit code: {}", status));
    }

    info!("NeoForge installation completed successfully");
    Ok(())
}

/// Verifies that a file is a valid JAR file by checking its ZIP structure
async fn verify_jar_file(jar_path: &Path) -> Result<bool> {
    info!("Verifying JAR file: {}", jar_path.display());

    // Check if the file exists
    if !jar_path.exists() {
        warn!("JAR file does not exist: {}", jar_path.display());
        return Ok(false);
    }

    // Check if the file has a non-zero size
    let metadata = tokio::fs::metadata(jar_path).await?;
    let file_size = metadata.len();
    if file_size == 0 {
        warn!("JAR file is empty: {}", jar_path.display());
        return Ok(false);
    }

    info!("JAR file size: {} bytes", file_size);

    // Try to read the file as a ZIP archive
    let zip_data = match tokio::fs::read(jar_path).await {
        Ok(data) => {
            if data.len() != file_size as usize {
                warn!("Read size ({} bytes) doesn't match file size ({} bytes)", data.len(), file_size);
                return Ok(false);
            }
            data
        },
        Err(e) => {
            warn!("Failed to read JAR file {}: {}", jar_path.display(), e);
            return Ok(false);
        }
    };

    // Check if the file starts with the ZIP magic number (PK\x03\x04)
    if zip_data.len() < 4 || &zip_data[0..4] != b"PK\x03\x04" {
        warn!("JAR file does not start with ZIP magic number");
        // Dump the first few bytes for debugging
        if !zip_data.is_empty() {
            let dump_size = std::cmp::min(16, zip_data.len());
            let hex_dump: Vec<String> = zip_data[..dump_size].iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            warn!("First {} bytes: {}", dump_size, hex_dump.join(" "));
        }
        return Ok(false);
    }

    // Try to parse the ZIP structure
    let result = tokio::task::spawn_blocking(move || -> Result<bool, String> {
        let cursor = std::io::Cursor::new(zip_data);
        match zip::ZipArchive::new(cursor) {
            Ok(mut archive) => {
                // Check if the archive has at least one entry
                if archive.len() == 0 {
                    Err("JAR file has no entries".to_string())
                } else {
                    // Try to read a few entries to verify the archive is not corrupted
                    let mut entry_count = 0;
                    for i in 0..std::cmp::min(5, archive.len()) {
                        match archive.by_index(i) {
                            Ok(_) => entry_count += 1,
                            Err(e) => return Err(format!("Failed to read entry {}: {}", i, e)),
                        }
                    }
                    Ok(true)
                }
            },
            Err(e) => {
                Err(format!("Failed to parse JAR file as ZIP: {}", e))
            }
        }
    }).await;

    match result {
        Ok(Ok(true)) => {
            info!("JAR file is valid");
            Ok(true)
        },
        Ok(Ok(false)) => {
            warn!("JAR file is invalid");
            Ok(false)
        },
        Ok(Err(e)) => {
            warn!("JAR file verification failed: {}", e);
            Ok(false)
        },
        Err(e) => {
            warn!("Error during JAR verification: {}", e);
            Ok(false)
        }
    }
}