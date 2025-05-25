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
use crate::config::{Config, save_config};
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

    // Game selector dropdown
    let game_selector_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
    game_selector_box.set_margin_top(10);
    game_selector_box.set_margin_bottom(10);
    game_selector_box.set_margin_start(10);
    game_selector_box.set_margin_end(10);

    let game_label = gtk::Label::new(Some("Current Game:"));
    game_label.set_halign(gtk::Align::Start);
    game_label.add_css_class("caption");
    game_selector_box.append(&game_label);

    // Create a dropdown for game selection
    let game_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let game_model = gtk::StringList::new(&[]);
    game_dropdown.set_model(Some(&game_model));
    game_dropdown.set_hexpand(true);
    game_selector_box.append(&game_dropdown);

    // Populate the dropdown with games from the config
    let config_ref = config.borrow();
    let selected_game_id = config_ref.selected_game.clone();
    let mut selected_index = 0;

    for (index, game) in config_ref.games.iter().enumerate() {
        game_model.append(&game.name);
        if let Some(selected_id) = &selected_game_id {
            if selected_id == &game.id {
                selected_index = index as u32;
            }
        }
    }

    if !config_ref.games.is_empty() {
        game_dropdown.set_selected(selected_index);
    }

    // Connect the dropdown to update the selected game
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();
    game_dropdown.connect_selected_notify(move |dropdown| {
        let selected = dropdown.selected();
        if selected == gtk::INVALID_LIST_POSITION || selected as usize >= config_clone.borrow().games.len() {
            return;
        }

        let game_id = config_clone.borrow().games[selected as usize].id.clone();
        let mut config_mut = config_clone.borrow_mut();
        config_mut.selected_game = Some(game_id.clone());

        // Save the config
        if let Err(e) = save_config(&config_mut) {
            error!("Failed to save config: {}", e);
            let toast = adw::Toast::new(&format!("Failed to save game selection: {}", e));
            toast_overlay_clone.add_toast(toast);
        } else {
            let toast = adw::Toast::new(&format!("Selected game: {}", config_mut.games[selected as usize].name));
            toast_overlay_clone.add_toast(toast);
        }
    });

    sidebar.append(&game_selector_box);

    // Account selector dropdown (only for Minecraft)
    let account_selector_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
    account_selector_box.set_margin_top(10);
    account_selector_box.set_margin_bottom(10);
    account_selector_box.set_margin_start(10);
    account_selector_box.set_margin_end(10);

    let account_label = gtk::Label::new(Some("Current Account:"));
    account_label.set_halign(gtk::Align::Start);
    account_label.add_css_class("caption");
    account_selector_box.append(&account_label);

    // Create a dropdown for account selection
    let account_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let account_model = gtk::StringList::new(&[]);
    account_dropdown.set_model(Some(&account_model));
    account_dropdown.set_hexpand(true);
    account_selector_box.append(&account_dropdown);

    // Check if we have an auth session
    let auth_guard = auth_session.lock().unwrap();
    let has_auth = auth_guard.is_some();

    if has_auth {
        // Get the current account name
        let current_account = auth_guard.as_ref().unwrap().minecraft_profile.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "Unknown".to_string());
        let is_offline = auth_guard.as_ref().unwrap().is_offline;

        // Add the current account to the model
        if is_offline {
            account_model.append(&format!("{} (Offline)", current_account));
        } else {
            account_model.append(&format!("{} (Microsoft)", current_account));
        }

        // Add an option to sign in with a different account
        account_model.append("Sign in with Microsoft...");
        account_model.append("Play in Offline Mode...");

        // Select the current account
        account_dropdown.set_selected(0);
    } else {
        // No auth session, add options to sign in
        account_model.append("Sign in with Microsoft...");
        account_model.append("Play in Offline Mode...");

        // Don't select anything
        account_dropdown.set_selected(gtk::INVALID_LIST_POSITION);
    }

    // Only show the account selector for Minecraft
    let is_minecraft = config.borrow().selected_game.as_ref().map_or(false, |id| {
        config.borrow().games.iter().find(|g| &g.id == id).map_or(false, |g| g.game_type == crate::config::GameType::Minecraft)
    });

    account_selector_box.set_visible(is_minecraft);

    // Connect the dropdown to handle account selection
    let auth_session_clone = auth_session.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();
    let stack_clone = content_container.clone();
    let config_clone = config.clone();

    account_dropdown.connect_selected_notify(move |dropdown| {
        let selected = dropdown.selected();
        if selected == gtk::INVALID_LIST_POSITION {
            return;
        }

        // Handle selection
        match selected {
            0 => {
                // Current account, do nothing
            },
            1 => {
                // Sign in with Microsoft
                // Show the login view
                let toast = adw::Toast::new("Switching to Microsoft login...");
                toast_overlay_clone.add_toast(toast);

                // Clear the current auth session
                *auth_session_clone.lock().unwrap() = None;

                // Navigate to the login view
                let app_window = window_clone.clone();
                let app = app_window.application().unwrap();

                // Create a new window for login
                let login_window = adw::ApplicationWindow::new(&app);
                login_window.set_title(Some("Sign in with Microsoft"));
                login_window.set_default_size(600, 700);
                login_window.set_modal(true);
                login_window.set_transient_for(Some(&window_clone));

                // Create a toast overlay for the login window
                let login_toast_overlay = adw::ToastOverlay::new();

                // Build the login view
                let login_view = super::login::build_login_view(
                    &login_window,
                    &login_toast_overlay,
                    Rc::new(crate::auth::AuthManager::new()),
                    auth_session_clone.clone(),
                    &gtk::Stack::new(), // Dummy stack, we'll handle navigation manually
                    config_clone.clone(),
                );

                login_toast_overlay.set_child(Some(&login_view));
                login_window.set_content(Some(&login_toast_overlay));

                // Show the login window
                login_window.present();

                // Reset the dropdown to the current account
                dropdown.set_selected(0);
            },
            2 => {
                // Play in Offline Mode
                // Show the offline login dialog
                let dialog = gtk::Dialog::new();
                dialog.set_title(Some("Offline Mode"));
                dialog.set_modal(true);
                dialog.set_transient_for(Some(&window_clone));
                dialog.set_default_width(350);

                // Create a header bar with title
                let header_bar = gtk::HeaderBar::new();
                header_bar.set_title_widget(Some(&gtk::Label::builder()
                    .label("Enter Username")
                    .css_classes(vec!["title-3"])
                    .build()));
                header_bar.set_show_title_buttons(false);
                dialog.set_titlebar(Some(&header_bar));

                // Create the content area
                let content_area = dialog.content_area();
                content_area.set_margin_top(24);
                content_area.set_margin_bottom(24);
                content_area.set_margin_start(24);
                content_area.set_margin_end(24);
                content_area.set_spacing(16);

                // Add instructions
                let instructions_label = gtk::Label::new(Some("Enter a username to use in offline mode:"));
                instructions_label.set_halign(gtk::Align::Start);
                content_area.append(&instructions_label);

                // Add username entry
                let username_entry = gtk::Entry::new();
                username_entry.set_placeholder_text(Some("Username"));
                username_entry.set_activates_default(true);
                content_area.append(&username_entry);

                // Add warning label
                let warning_label = gtk::Label::new(Some("Warning: Offline mode only works for singleplayer or cracked servers."));
                warning_label.add_css_class("caption");
                warning_label.set_halign(gtk::Align::Start);
                content_area.append(&warning_label);

                // Add action buttons
                dialog.add_button("Cancel", gtk::ResponseType::Cancel);
                let ok_button = dialog.add_button("Play Offline", gtk::ResponseType::Ok);
                dialog.set_default_response(gtk::ResponseType::Ok);

                // Clone references for the response handler
                let auth_session = auth_session_clone.clone();
                let toast_overlay = toast_overlay_clone.clone();
                let username_entry_clone = username_entry.clone();
                let dropdown_clone = dropdown.clone();

                // Connect the response signal
                dialog.connect_response(move |dialog, response| {
                    dialog.destroy();

                    if response == gtk::ResponseType::Ok {
                        let username = username_entry_clone.text().to_string();

                        // Validate username
                        if username.is_empty() {
                            let toast = adw::Toast::new("Username cannot be empty");
                            toast_overlay.add_toast(toast);
                            return;
                        }

                        // Create an offline session
                        let auth_manager = crate::auth::AuthManager::new();
                        match auth_manager.create_offline_session(&username) {
                            Ok(session) => {
                                // Store the auth session
                                *auth_session.lock().unwrap() = Some(session);

                                // Show a success toast
                                let toast = adw::Toast::new(&format!("Playing as {} (offline mode)", username));
                                toast_overlay.add_toast(toast);

                                // Reset the dropdown to show the new account
                                dropdown_clone.set_selected(0);
                            }
                            Err(e) => {
                                // Show error toast
                                let toast = adw::Toast::new(&format!("Failed to create offline session: {}", e));
                                toast_overlay.add_toast(toast);
                            }
                        }
                    } else {
                        // Reset the dropdown to the current account
                        dropdown_clone.set_selected(0);
                    }
                });

                // Show the dialog
                dialog.present();
            },
            _ => {
                // Unknown option
                let toast = adw::Toast::new("Unknown account option selected");
                toast_overlay_clone.add_toast(toast);
            }
        }
    });

    // Update account selector visibility when game changes
    let account_selector_box_clone = account_selector_box.clone();
    let config_clone = config.clone();
    game_dropdown.connect_selected_notify(move |dropdown| {
        let selected = dropdown.selected();
        if selected == gtk::INVALID_LIST_POSITION || selected as usize >= config_clone.borrow().games.len() {
            return;
        }

        let game = &config_clone.borrow().games[selected as usize];
        let is_minecraft = game.game_type == crate::config::GameType::Minecraft;
        account_selector_box_clone.set_visible(is_minecraft);
    });

    sidebar.append(&account_selector_box);

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    sidebar.append(&separator);

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
