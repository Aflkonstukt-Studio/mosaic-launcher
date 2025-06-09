// Games module for Mosaic Launcher
// This file defines the interfaces for game plugins

pub mod minecraft;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use serde::{Serialize, Deserialize};
use crate::config::{Config, Profile};
use crate::file_manager::{FileManager, DownloadProgress};
use crate::games::minecraft::auth::AuthSession;

/// Enum representing the UI type for a game plugin
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePluginUIType {
    /// Use the predefined UI (basic play button, profiles, mods if supported)
    Predefined,
    /// Modify the UI using JSON
    JSONCustomized,
    /// Create a custom UI using GTK4
    CustomGTK,
}

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

    /// Get available modloader versions for a specific game version
    fn get_modloader_versions(&mut self, mod_loader_type: &str, game_version: &str) -> Result<Vec<String>>;

    /// Install a specific version
    fn install_version(&self, version_id: &str, progress_callback: Box<dyn Fn(DownloadProgress) + Send + Sync + 'static>) -> Result<()>;

    /// Get the UI type for this game plugin
    fn get_ui_type(&self) -> GamePluginUIType {
        // Default to predefined UI
        GamePluginUIType::Predefined
    }

    /// Get the JSON UI customization for this game plugin
    /// This is only used if get_ui_type() returns GamePluginUIType::JSONCustomized
    fn get_json_ui_customization(&self) -> Option<serde_json::Value> {
        None
    }

    /// Get the mod search and download URL for this game plugin
    /// This is used by the predefined UI to search for and download mods
    fn get_mod_search_url(&self) -> Option<String> {
        None
    }

    /// Get the mod download URL for this game plugin
    /// This is used by the predefined UI to download mods
    fn get_mod_download_url(&self) -> Option<String> {
        None
    }

    /// Handle game selection
    /// This method is called when the game is selected in the game selector
    /// It should handle any game-specific UI or logic that needs to happen when the game is selected
    /// For example, showing a login screen for games that require authentication
    /// 
    /// Parameters:
    /// - window: The application window
    /// - toast_overlay: The toast overlay for showing notifications
    /// - stack: The stack for navigating between views
    /// - auth_session: The authentication session
    /// - config: The application configuration
    /// 
    /// Returns:
    /// - Ok(()): If the game selection was handled successfully
    /// - Err(e): If there was an error handling the game selection
    fn handle_game_selection(
        &self,
        window: &gtk4::Window,
        toast_overlay: &libadwaita::ToastOverlay,
        stack: &gtk4::Stack,
        auth_session: Arc<Mutex<Option<AuthSession>>>,
        config: Rc<RefCell<Config>>,
    ) -> Result<()>;
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
    fn create(&self, config: Config, file_manager: Rc<FileManager>) -> Box<dyn GamePlugin>;
}

/// Manager for game plugins
pub struct GamePluginManager {
    plugins: Vec<Box<dyn GamePlugin>>,
    factories: Vec<Box<dyn GamePluginFactory>>,
    config: Rc<RefCell<Config>>,
    file_manager: Rc<FileManager>,
}

impl GamePluginManager {
    /// Create a new game plugin manager
    pub fn new(config: Rc<RefCell<Config>>, file_manager: Rc<FileManager>) -> Self {
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
        let config = self.config.borrow().clone();

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
