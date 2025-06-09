// Minecraft game plugin for Mosaic Launcher

mod models;
mod manager;
mod modloaders;
mod versions;
mod launcher;
pub mod auth;
pub mod ui;

use anyhow::Result;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::cell::RefCell;
use gtk4 as gtk;
use libadwaita as adw;
use crate::config::{Config, Profile};
use crate::file_manager::{DownloadProgress, FileManager};
use self::auth::{AuthManager, AuthSession};
use crate::games::{GamePlugin, GamePluginFactory, GamePluginUIType};
use crate::mods::ModManager;
use crate::games::minecraft::ui::login::build_login_view;

pub use self::manager::MinecraftManager;
pub use self::models::VersionManifest;

/// Minecraft game plugin
pub struct MinecraftPlugin {
    manager: MinecraftManager,
    mod_manager: MinecraftManager,
    id: String,
    name: String,
    ui_type: GamePluginUIType,
}

impl MinecraftPlugin {
    /// Create a new Minecraft game plugin
    pub fn new(config: Config, file_manager: Rc<FileManager>) -> Self {
        Self {
            manager: MinecraftManager::new(config.clone(), file_manager.clone()),
            mod_manager: MinecraftManager::new(config, file_manager),
            id: "minecraft".to_string(),
            name: "Minecraft".to_string(),
            ui_type: GamePluginUIType::Predefined,
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

    fn get_ui_type(&self) -> GamePluginUIType {
        GamePluginUIType::Predefined
    }

    fn get_mod_search_url(&self) -> Option<String> {
        Some("https://api.modrinth.com/v2/search".to_string())
    }

    fn get_mod_download_url(&self) -> Option<String> {
        Some("https://api.modrinth.com/v2/project".to_string())
    }

    fn handle_game_selection(
        &self,
        window: &gtk::Window,
        toast_overlay: &adw::ToastOverlay,
        stack: &gtk::Stack,
        auth_session: Arc<Mutex<Option<AuthSession>>>,
        config: Rc<RefCell<Config>>,
    ) -> Result<()> {
        // Create a new AuthManager
        let auth_manager = Rc::new(AuthManager::new());

        // Build the login view
        let login_view = build_login_view(
            window,
            toast_overlay,
            auth_manager,
            auth_session,
            stack,
            config,
        );

        // Create a name for the login view in the stack
        let login_view_name = format!("{}_login", self.get_id());

        // Add the login view to the stack
        stack.add_named(&login_view, Some(&login_view_name));

        // Show the login view
        stack.set_visible_child_name(&login_view_name);

        Ok(())
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

    fn create(&self, config: Config, file_manager: Rc<FileManager>) -> Box<dyn GamePlugin> {
        Box::new(MinecraftPlugin::new(config, file_manager))
    }
}
