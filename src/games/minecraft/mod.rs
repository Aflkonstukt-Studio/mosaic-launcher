// Minecraft game plugin for Mosaic Launcher

mod models;
mod manager;
mod modloaders;
mod versions;
mod launcher;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use crate::config::{Config, Profile};
use crate::file_manager::{FileManager, DownloadProgress};
use crate::auth::AuthSession;
use crate::games::{GamePlugin, GamePluginFactory};

pub use self::manager::MinecraftManager;
pub use self::models::VersionManifest;

/// Minecraft game plugin
pub struct MinecraftPlugin {
    manager: MinecraftManager,
    id: String,
    name: String,
}

impl MinecraftPlugin {
    /// Create a new Minecraft game plugin
    pub fn new(config: Config, file_manager: FileManager) -> Self {
        Self {
            manager: MinecraftManager::new(config, file_manager),
            id: "minecraft".to_string(),
            name: "Minecraft".to_string(),
        }
    }
}

impl GamePlugin for MinecraftPlugin {
    fn get_id(&self) -> &str {
        &self.id
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_icon_name(&self) -> &str {
        "applications-games-symbolic"
    }

    fn get_game_directory(&self) -> PathBuf {
        self.manager.get_minecraft_directory()
    }

    fn set_game_directory(&mut self, directory: PathBuf) {
        self.manager.set_minecraft_directory(directory);
    }

    fn get_profiles(&self) -> Vec<Profile> {
        self.manager.get_profiles()
    }

    fn add_profile(&mut self, profile: Profile) {
        self.manager.add_profile(profile);
    }

    fn update_profile(&mut self, profile: Profile) -> Result<()> {
        self.manager.update_profile(profile)
    }

    fn delete_profile(&mut self, profile_id: &str) -> Result<()> {
        self.manager.delete_profile(profile_id)
    }

    fn launch_game(
        &self,
        profile: &Profile,
        auth_session: &AuthSession,
        progress_callback: Box<dyn Fn(DownloadProgress) + Send + Sync + 'static>,
    ) -> Result<u32> {
        let callback = Arc::new(progress_callback);
        let callback_clone = Arc::clone(&callback);
        let wrapper = move |progress: DownloadProgress| {
            callback_clone(progress);
        };
        // Create a tokio runtime to run the async method
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.manager.launch_game(profile, auth_session, wrapper))
    }

    fn is_version_installed(&self, version_id: &str) -> bool {
        self.manager.is_version_installed(version_id)
    }

    fn get_available_versions(&self) -> Result<Vec<String>> {
        let manifest = self.manager.get_version_manifest()?;
        let versions = manifest.versions.iter().map(|v| v.id.clone()).collect();
        Ok(versions)
    }

    fn get_modloader_versions(&mut self, mod_loader_type: &str, game_version: &str) -> Result<Vec<String>> {
        self.manager.get_modloader_versions(mod_loader_type, game_version)
    }

    fn install_version(
        &self,
        version_id: &str,
        progress_callback: Box<dyn Fn(DownloadProgress) + Send + Sync + 'static>,
    ) -> Result<()> {
        let callback = Arc::new(progress_callback);
        let callback_clone = Arc::clone(&callback);
        let wrapper = move |progress: DownloadProgress| {
            callback_clone(progress);
        };

        // Create a tokio runtime to run the async method
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(self.manager.install_version(version_id, wrapper))
    }
}

/// Factory for creating Minecraft game plugins
pub struct MinecraftPluginFactory;

impl GamePluginFactory for MinecraftPluginFactory {
    fn get_id(&self) -> &str {
        "minecraft"
    }

    fn get_name(&self) -> &str {
        "Minecraft"
    }

    fn get_icon_name(&self) -> &str {
        "applications-games-symbolic"
    }

    fn create(&self, config: Config, file_manager: FileManager) -> Box<dyn GamePlugin> {
        Box::new(MinecraftPlugin::new(config, file_manager))
    }
}
