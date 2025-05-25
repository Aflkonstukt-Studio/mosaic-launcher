// Games module for Mosaic Launcher
// This file defines the interfaces for game plugins

pub mod minecraft;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::config::{Config, Profile};
use crate::file_manager::{FileManager, DownloadProgress};
use crate::auth::AuthSession;

/// Trait that all game plugins must implement
pub trait GamePlugin: Send + Sync {
    /// Get the ID of the game
    fn get_id(&self) -> &str;

    /// Get the name of the game
    fn get_name(&self) -> &str;

    /// Get the icon name for the game
    fn get_icon_name(&self) -> &str;

    /// Get the game directory
    fn get_game_directory(&self) -> PathBuf;

    /// Set the game directory
    fn set_game_directory(&mut self, directory: PathBuf);

    /// Get the profiles for this game
    fn get_profiles(&self) -> Vec<Profile>;

    /// Add a profile
    fn add_profile(&mut self, profile: Profile);

    /// Update a profile
    fn update_profile(&mut self, profile: Profile) -> Result<()>;

    /// Delete a profile
    fn delete_profile(&mut self, profile_id: &str) -> Result<()>;

    /// Launch the game with the specified profile
    fn launch_game(&self, profile: &Profile, auth_session: &AuthSession, progress_callback: Box<dyn Fn(DownloadProgress) + Send + Sync + 'static>) -> Result<u32>;

    /// Check if a version is installed
    fn is_version_installed(&self, version_id: &str) -> bool;

    /// Get available versions
    fn get_available_versions(&self) -> Result<Vec<String>>;

    /// Get available modloader versions for a specific Minecraft version
    fn get_modloader_versions(&mut self, mod_loader_type: &str, game_version: &str) -> Result<Vec<String>>;

    /// Install a specific version
    fn install_version(&self, version_id: &str, progress_callback: Box<dyn Fn(DownloadProgress) + Send + Sync + 'static>) -> Result<()>;
}

/// Factory for creating game plugins
pub trait GamePluginFactory: Send + Sync {
    /// Get the ID of the game
    fn get_id(&self) -> &str;

    /// Get the name of the game
    fn get_name(&self) -> &str;

    /// Get the icon name for the game
    fn get_icon_name(&self) -> &str;

    /// Create a new instance of the game plugin
    fn create(&self, config: Config, file_manager: FileManager) -> Box<dyn GamePlugin>;
}

/// Manager for game plugins
pub struct GamePluginManager {
    plugins: Vec<Box<dyn GamePlugin>>,
    factories: Vec<Box<dyn GamePluginFactory>>,
    config: Arc<Mutex<Config>>,
    file_manager: FileManager,
}

impl GamePluginManager {
    /// Create a new game plugin manager
    pub fn new(config: Arc<Mutex<Config>>, file_manager: FileManager) -> Self {
        Self {
            plugins: Vec::new(),
            factories: Vec::new(),
            config,
            file_manager,
        }
    }

    /// Register a game plugin factory
    pub fn register_factory(&mut self, factory: Box<dyn GamePluginFactory>) {
        self.factories.push(factory);
    }

    /// Initialize plugins from config
    pub fn initialize_plugins(&mut self) {
        let config = self.config.lock().unwrap().clone();

        for factory in &self.factories {
            let plugin = factory.create(config.clone(), self.file_manager.clone());
            self.plugins.push(plugin);
        }
    }

    /// Get a plugin by ID
    pub fn get_plugin(&self, id: &str) -> Option<&Box<dyn GamePlugin>> {
        self.plugins.iter().find(|p| p.get_id() == id)
    }

    /// Get a mutable plugin by ID
    pub fn get_plugin_mut(&mut self, id: &str) -> Option<&mut Box<dyn GamePlugin>> {
        self.plugins.iter_mut().find(|p| p.get_id() == id)
    }

    /// Get all plugins
    pub fn get_plugins(&self) -> &[Box<dyn GamePlugin>] {
        &self.plugins
    }

    /// Get all plugin factories
    pub fn get_factories(&self) -> &[Box<dyn GamePluginFactory>] {
        &self.factories
    }
}
