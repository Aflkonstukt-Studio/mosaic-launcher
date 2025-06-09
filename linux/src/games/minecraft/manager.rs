// Minecraft manager for the Minecraft game plugin

use anyhow::{Result, anyhow};
use reqwest::blocking::Client as HttpClient;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;
use log::{info, warn, error, debug};

use crate::games::minecraft::auth::AuthSession;
use crate::config::{Config, Profile, ModLoader};
use crate::file_manager::{FileManager, DownloadProgress};

use super::models::{VersionManifest, VersionDetails, VersionInfo};
use super::modloaders;
use super::versions;
use super::launcher;

// Minecraft version manifest URL
const VERSION_MANIFEST_URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";

/// Minecraft manager
pub struct MinecraftManager {
    http_client: HttpClient,
    file_manager: FileManager,
    config: Config,
    // Cache for modloader versions
    modloader_versions_cache: HashMap<String, Vec<String>>,
    minecraft_directory: PathBuf,
}

impl MinecraftManager {
    /// Create a new Minecraft manager
    pub fn new(config: Config, file_manager: Rc<FileManager>) -> Self {
        // Get the Minecraft directory from the config
        let minecraft_directory = Self::get_minecraft_directory_from_config(&config);

        Self {
            http_client: HttpClient::new(),
            file_manager: (*file_manager).clone(),
            config,
            modloader_versions_cache: HashMap::new(),
            minecraft_directory,
        }
    }

    /// Get the Minecraft directory
    pub fn get_minecraft_directory(&self) -> PathBuf {
        self.minecraft_directory.clone()
    }

    /// Set the Minecraft directory
    pub fn set_minecraft_directory(&mut self, directory: PathBuf) {
        self.minecraft_directory = directory;
    }

    /// Get the config
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// Get the profiles for this game
    pub fn get_profiles(&self) -> Vec<Profile> {
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

        game.profiles.clone()
    }

    /// Add a profile
    pub fn add_profile(&mut self, profile: Profile) {
        // Get the selected game ID
        let selected_game_id = self.config.selected_game.clone().unwrap_or_else(|| {
            if !self.config.games.is_empty() {
                self.config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game_index = self.config.games.iter()
            .position(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !self.config.games.is_empty() {
                    0
                } else {
                    panic!("No games found in config")
                }
            });

        // Add the profile
        self.config.games[game_index].profiles.push(profile);
    }

    /// Update a profile
    pub fn update_profile(&mut self, profile: Profile) -> Result<()> {
        // Get the selected game ID
        let selected_game_id = self.config.selected_game.clone().unwrap_or_else(|| {
            if !self.config.games.is_empty() {
                self.config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game_index = self.config.games.iter()
            .position(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !self.config.games.is_empty() {
                    0
                } else {
                    panic!("No games found in config")
                }
            });

        // Find the profile
        let profile_index = self.config.games[game_index].profiles.iter()
            .position(|p| p.id == profile.id)
            .ok_or_else(|| anyhow!("Profile not found"))?;

        // Update the profile
        self.config.games[game_index].profiles[profile_index] = profile;

        Ok(())
    }

    /// Delete a profile
    pub fn delete_profile(&mut self, profile_id: &str) -> Result<()> {
        // Get the selected game ID
        let selected_game_id = self.config.selected_game.clone().unwrap_or_else(|| {
            if !self.config.games.is_empty() {
                self.config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game_index = self.config.games.iter()
            .position(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !self.config.games.is_empty() {
                    0
                } else {
                    panic!("No games found in config")
                }
            });

        // Find the profile
        let profile_index = self.config.games[game_index].profiles.iter()
            .position(|p| p.id == profile_id)
            .ok_or_else(|| anyhow!("Profile not found"))?;

        // Delete the profile
        self.config.games[game_index].profiles.remove(profile_index);

        Ok(())
    }

    /// Get the version manifest
    pub fn get_version_manifest(&self) -> Result<VersionManifest> {
        info!("Fetching Minecraft version manifest");
        let manifest = self.http_client
            .get(VERSION_MANIFEST_URL)
            .send()?
            .json::<VersionManifest>()?;

        Ok(manifest)
    }

    /// Get version details
    pub fn get_version_details(&self, version_info: &VersionInfo) -> Result<VersionDetails> {
        info!("Fetching details for Minecraft version {}", version_info.id);
        let details = self.http_client
            .get(&version_info.url)
            .send()?
            .json::<VersionDetails>()?;

        Ok(details)
    }

    /// Check if a version is installed
    pub fn is_version_installed(&self, version_id: &str) -> bool {
        versions::is_version_installed(&self.minecraft_directory, version_id)
    }

    /// Install a version
    pub async fn install_version(&self, version_id: &str, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<()> {
        // Get the version manifest
        let manifest = self.get_version_manifest()?;

        // Find the version
        let version_info = manifest.versions.iter()
            .find(|v| v.id == version_id)
            .ok_or_else(|| anyhow!("Version {} not found in manifest", version_id))?;

        // Get the version details
        let version_details = self.get_version_details(version_info)?;

        // Download the version
        versions::download_version(
            &self.file_manager,
            &self.minecraft_directory,
            &version_details,
            progress_callback,
        ).await
    }

    /// Get available modloader versions for a specific Minecraft version
    pub fn get_modloader_versions(&mut self, mod_loader_type: &str, game_version: &str) -> Result<Vec<String>> {
        // Create a cache key
        let cache_key = format!("{}_{}", mod_loader_type, game_version);

        // Check if we have cached versions
        if let Some(versions) = self.modloader_versions_cache.get(&cache_key) {
            return Ok(versions.clone());
        }

        // Fetch versions based on modloader type
        let versions = match mod_loader_type {
            "forge" => modloaders::fetch_forge_versions(game_version)?,
            "fabric" => modloaders::fetch_fabric_versions(game_version)?,
            "quilt" => modloaders::fetch_quilt_versions(game_version)?,
            "neoforge" => modloaders::fetch_neoforge_versions(game_version)?,
            _ => vec![],
        };

        // Cache the versions
        self.modloader_versions_cache.insert(cache_key, versions.clone());

        Ok(versions)
    }

    /// Launch the game
    pub async fn launch_game(&self, profile: &Profile, auth_session: &AuthSession, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static + Clone) -> Result<u32> {
        // Get the version manifest
        let manifest = self.get_version_manifest()?;

        // Find the version
        let version_info = manifest.versions.iter()
            .find(|v| v.id == profile.version)
            .ok_or_else(|| anyhow!("Version {} not found in manifest", profile.version))?;

        // Get the version details
        let version_details = self.get_version_details(version_info)?;

        // Get the Java path
        let java_path = launcher::get_java_path()?;

        // Launch the game
        launcher::launch_game(
            &self.minecraft_directory,
            profile,
            auth_session,
            &version_details,
            &java_path,
        ).await
    }

    /// Helper function to get the Minecraft directory from the config
    fn get_minecraft_directory_from_config(config: &Config) -> PathBuf {
        // Get the selected game ID
        let selected_game_id = config.selected_game.clone().unwrap_or_else(|| {
            if !config.games.is_empty() {
                config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game = config.games.iter()
            .find(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !config.games.is_empty() {
                    &config.games[0]
                } else {
                    panic!("No games found in config")
                }
            });

        game.game_directory.clone()
    }
}
