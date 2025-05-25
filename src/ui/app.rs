use gtk4 as gtk;
use gtk::prelude::*;
use gtk::MessageDialog;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use anyhow::{Result, anyhow};
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::auth::{AuthManager, AuthSession};
use crate::config::{Config, Profile, ModLoader, save_config};
use crate::minecraft::{MinecraftManager, VersionManifest, VersionInfo};
use crate::mods::{ModManager, ModSearchParams, ModSearchResult, ModVersionInfo, ModSortField, SortOrder};
use crate::file_manager::{FileManager, DownloadProgress};

use super::login::build_login_view;
use super::main_view::build_main_view;

pub struct MosaicApp {
    app: adw::Application,
    config: Rc<RefCell<Config>>,
    auth_manager: Rc<AuthManager>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
}

impl MosaicApp {
    pub fn new(config: &Config) -> Self {
        // Create the application
        info!("Creating application");
        let app = adw::Application::new(Some("xyz.aflkonstukt.launcher"), Default::default());

        // Create shared resources
        let config = Rc::new(RefCell::new(config.clone()));
        let auth_manager = Rc::new(AuthManager::new());

        // Create a single FileManager instance
        let file_manager_instance = FileManager::new();

        // Use the FileManager instance for MinecraftManager and ModManager
        let minecraft_manager = Arc::new(Mutex::new(MinecraftManager::new(config.borrow().clone(), file_manager_instance.clone())));
        let mod_manager = Rc::new(RefCell::new(ModManager::new(config.borrow().clone(), file_manager_instance.clone())));

        // Wrap the FileManager instance in an Rc for the MosaicApp struct
        let file_manager = Rc::new(file_manager_instance);
        let auth_session = Arc::new(Mutex::new(None));
        let version_manifest = Arc::new(Mutex::new(None));

        info!("Application created");

        // Create the application
        MosaicApp {
            app,
            config,
            auth_manager,
            minecraft_manager,
            mod_manager,
            file_manager,
            auth_session,
            version_manifest,
        }
    }

    pub fn run(&self) -> i32 {
        // Connect the activate signal
        let config = self.config.clone();
        let auth_manager = self.auth_manager.clone();
        let minecraft_manager = self.minecraft_manager.clone();
        let mod_manager = self.mod_manager.clone();
        let file_manager = self.file_manager.clone();
        let auth_session = self.auth_session.clone();
        let version_manifest = self.version_manifest.clone();

        info!("Connecting activate signal");
        self.app.connect_activate(move |app| {
            // Build the main window
            let window = build_main_window(
                app,
                config.clone(),
                auth_manager.clone(),
                minecraft_manager.clone(),
                mod_manager.clone(),
                file_manager.clone(),
                auth_session.clone(),
                version_manifest.clone(),
            );
            
            // Show the window
            info!("Showing main window");
            window.present();
        });

        // Run the application
        self.app.run().into()
    }
}

fn build_main_window(
    app: &adw::Application,
    config: Rc<RefCell<Config>>,
    auth_manager: Rc<AuthManager>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
) -> adw::ApplicationWindow {
    // Create the main window
    info!("Building main application window");
    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Mosaic Launcher"));
    window.set_default_size(1200, 800);

    // Create a toast overlay for notifications
    let toast_overlay = adw::ToastOverlay::new();

    // Create a stack to switch between login and main views
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);

    // Build the login view
    let login_view = build_login_view(
        &window,
        &toast_overlay,
        auth_manager.clone(),
        auth_session.clone(),
        &stack,
        config.clone(),
    );
    stack.add_named(&login_view, Some("login"));
    info!("Login view built");

    // Build the main view
    let main_view = build_main_view(
        &window,
        &toast_overlay,
        config.clone(),
        minecraft_manager.clone(),
        mod_manager.clone(),
        file_manager.clone(),
        auth_session.clone(),
        version_manifest.clone(),
    );
    stack.add_named(&main_view, Some("main"));

    info!("Main view built");

    // Start with the login view
    stack.set_visible_child_name("login");

    info!("Stack built");

    // Add the stack to the toast overlay
    toast_overlay.set_child(Some(&stack));

    // Add the toast overlay to the window
    window.set_content(Some(&toast_overlay));

    // Show the window
    info!("Main application window built");

    window
}
