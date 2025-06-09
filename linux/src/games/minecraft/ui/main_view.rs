// Minecraft main view UI component for Mosaic Launcher

use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use log::{info, warn, error, debug};

use crate::config::Config;
use crate::games::GamePluginManager;
use crate::file_manager::FileManager;
use crate::games::minecraft::auth::AuthSession;
use crate::games::minecraft::ui::sidebar::build_sidebar;

/// Build the main view for Minecraft
pub fn build_main_view(
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
) -> gtk::Box {
    info!("Building Minecraft main view");

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
        game_plugin_manager.clone(),
        file_manager.clone(),
        content_container.clone(),
        auth_session.clone(),
    );

    // Add sidebar and content to main container
    main_box.append(&sidebar);
    main_box.append(&content_container);

    main_box
}
