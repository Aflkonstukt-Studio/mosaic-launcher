use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use log::{info, warn, error, debug};

use crate::auth::AuthSession;
use crate::config::Config;
use crate::games::minecraft::{MinecraftManager, VersionManifest};
use crate::mods::ModManager;
use crate::file_manager::FileManager;

use super::play::build_play_view;
use super::profiles::build_profiles_view;
use super::mods::build_mods_view;
use super::settings::build_settings_view;
use super::game_selector::build_game_selector;

pub fn build_main_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
) -> gtk::Box {
    info!("Building main view");

    // Create main container
    let main_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    // Create content container
    let content_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_container.set_hexpand(true);

    // Add initial placeholder to content container
    let placeholder = gtk::Label::new(Some("Select an option from the sidebar"));
    placeholder.add_css_class("title-1");
    placeholder.set_vexpand(true);
    content_container.append(&placeholder);

    // Build sidebar
    let sidebar = build_sidebar(
        window,
        toast_overlay,
        config.clone(),
        minecraft_manager.clone(),
        mod_manager.clone(),
        file_manager.clone(),
        auth_session.clone(),
        version_manifest.clone(),
        content_container.clone(),
    );

    // Add sidebar and content to main container
    main_box.append(&sidebar);
    main_box.append(&content_container);

    // Load version manifest in background thread
    load_version_manifest_async(
        minecraft_manager,
        version_manifest,
        toast_overlay,
    );

    main_box
}

fn load_version_manifest_async(
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
    toast_overlay: &adw::ToastOverlay,
) {
    let toast_overlay = toast_overlay.clone();
    let mut error = false;

    std::thread::spawn(move || {
        info!("Starting async version manifest load");
        match minecraft_manager.lock().unwrap().get_version_manifest() {
            Ok(manifest) => {
                *version_manifest.lock().unwrap() = Some(manifest);
                info!("Version manifest loaded successfully");
            }
            Err(e) => {
                error!("Failed to load version manifest: {}", e);
                error = true;
            }
        }
    });

    if error {
        let toast = adw::Toast::new("Failed to load version manifest");
        toast.set_timeout(5);
        toast_overlay.add_toast(toast);
    }
}

fn build_sidebar(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
    content_container: gtk::Box,
) -> gtk::Box {
    info!("Building sidebar");

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.set_width_request(250);
    sidebar.add_css_class("sidebar");

    // Header
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Mosaic"))));
    header.set_show_end_title_buttons(false);
    sidebar.append(&header);

    // Navigation list
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");
    sidebar.append(&list_box);

    // Navigation items
    let items = [
        ("Games", "applications-games-symbolic"),
        ("Play", "media-playback-start-symbolic"),
        ("Profiles", "view-list-symbolic"),
        ("Mods", "view-grid-symbolic"),
        ("Settings", "preferences-system-symbolic"),
    ];

    for (title, icon) in items {
        let row = adw::ActionRow::new();
        row.set_title(title);
        row.set_icon_name(Some(icon));
        list_box.append(&row);
    }

    // Connect selection handler
    list_box.connect_row_selected(clone!(
        @strong window, 
        @strong toast_overlay,
        @strong config,
        @strong minecraft_manager,
        @strong mod_manager,
        @strong file_manager,
        @strong auth_session,
        @strong version_manifest,
        @strong content_container => 
        move |_, row| {
            handle_sidebar_selection(
                row,
                &window,
                &toast_overlay,
                config.clone(),
                minecraft_manager.clone(),
                mod_manager.clone(),
                file_manager.clone(),
                auth_session.clone(),
                version_manifest.clone(),
                &content_container,
            );
        }
    ));

    // Initial selection
    list_box.connect_map(move |list_box| {
        if let Some(row) = list_box.row_at_index(0) {
            list_box.select_row(Some(&row));
        }
    });

    sidebar
}

fn handle_sidebar_selection(
    row: Option<&gtk::ListBoxRow>,
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
    content_container: &gtk::Box,
) {
    if let Some(row) = row {
        // Remove existing content
        while let Some(old_content) = content_container.first_child() {
            content_container.remove(&old_content);
        }

        // Build and add new content
        let new_content = match row.index() {
            0 => build_game_selector(
                window,
                toast_overlay,
                config.clone(),
            ).upcast(),
            1 => build_play_view(
                window,
                toast_overlay,
                config,
                minecraft_manager,
                auth_session,
                version_manifest,
            ).upcast(),
            2 => build_profiles_view(
                window,
                toast_overlay,
                config,
                version_manifest,
            ).upcast(),
            3 => build_mods_view(
                window,
                toast_overlay,
                config,
                mod_manager,
                file_manager,
            ).upcast(),
            4 => build_settings_view(
                window,
                toast_overlay,
                config,
            ).upcast(),
            _ => {
                warn!("Unknown sidebar selection: {}", row.index());
                gtk::Box::new(gtk::Orientation::Vertical, 0)
            }
        };

        content_container.append(&new_content);
    }
}
