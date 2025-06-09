// Minecraft sidebar UI component for Mosaic Launcher

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

use crate::config::{Config, save_config, GameType};
use crate::games::GamePluginManager;
use crate::file_manager::FileManager;
use crate::games::minecraft::auth::AuthSession;
// We'll implement our own versions of these views

// Navigation items for the sidebar
const SIDEBAR_ITEMS: [(&str, &str); 5] = [
    ("Games", "applications-games-symbolic"),
    ("Play", "media-playback-start-symbolic"),
    ("Profiles", "view-list-symbolic"),
    ("Mods", "view-grid-symbolic"),
    ("Settings", "preferences-system-symbolic"),
];

/// Build the sidebar for the Minecraft UI
pub fn build_sidebar(
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
    file_manager: Rc<FileManager>,
    content_container: gtk::Box,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
) -> gtk::Box {
    info!("Building Minecraft sidebar");

    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.set_width_request(250);
    sidebar.add_css_class("sidebar");

    // Header
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Minecraft"))));
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

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    sidebar.append(&separator);

    // Navigation list
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");
    sidebar.append(&list_box);

    // Use the sidebar items defined at the top level
    for (title, icon) in SIDEBAR_ITEMS {
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
        @strong file_manager,
        @strong game_plugin_manager,
        @strong content_container,
        @strong auth_session => 
        move |_, row| {
            handle_sidebar_selection(
                row,
                &window,
                &toast_overlay,
                config.clone(),
                file_manager.clone(),
                game_plugin_manager.clone(),
                &content_container,
                auth_session.clone(),
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

/// Handle sidebar selection
fn handle_sidebar_selection(
    row: Option<&gtk::ListBoxRow>,
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    file_manager: Rc<FileManager>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
    content_container: &gtk::Box,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
) {
    if let Some(row) = row {
        // Remove existing content
        while let Some(old_content) = content_container.first_child() {
            content_container.remove(&old_content);
        }

        // Get the selected game plugin
        let selected_game_id = config.borrow().selected_game.clone();
        let plugin_manager = game_plugin_manager.lock().unwrap();

        // Build and add new content
        let new_content = match row.index() {
            0 => {
                // Simple game selector view
                let game_selector_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                game_selector_box.set_margin_top(20);
                game_selector_box.set_margin_bottom(20);
                game_selector_box.set_margin_start(20);
                game_selector_box.set_margin_end(20);

                // Add a header
                let header = gtk::Label::new(Some("Select Game"));
                header.add_css_class("title-2");
                header.set_halign(gtk::Align::Start);
                game_selector_box.append(&header);

                // Add a separator
                let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                separator.set_margin_top(10);
                separator.set_margin_bottom(10);
                game_selector_box.append(&separator);

                // Add a message
                let message = gtk::Label::new(Some("Select a game from the dropdown above."));
                message.set_halign(gtk::Align::Start);
                game_selector_box.append(&message);

                game_selector_box
            },
            1 => {
                // Play view
                if let Some(game_id) = &selected_game_id {
                    if let Some(plugin) = plugin_manager.get_plugin(game_id) {
                        if game_id == "minecraft" {
                            // For Minecraft, use the play view from the Minecraft plugin
                            // Use the existing auth_session from the app
                            let auth_session_clone = Arc::clone(&auth_session);

                            // Build the play view
                            let play_view = crate::games::minecraft::ui::ui::build_play_view(
                                window.upcast_ref::<gtk::Window>(),
                                toast_overlay,
                                plugin.as_ref(),
                                auth_session_clone,
                            );

                            play_view.upcast()
                        } else {
                            // For other games, use a placeholder
                            let plugin_ui = gtk::Box::new(gtk::Orientation::Vertical, 10);
                            plugin_ui.set_margin_top(20);
                            plugin_ui.set_margin_bottom(20);
                            plugin_ui.set_margin_start(20);
                            plugin_ui.set_margin_end(20);

                            // Add a header
                            let header = gtk::Label::new(Some(&format!("{} - {}", plugin.get_name(), SIDEBAR_ITEMS[row.index() as usize].0)));
                            header.add_css_class("title-2");
                            header.set_halign(gtk::Align::Start);
                            plugin_ui.append(&header);

                            // Add a separator
                            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                            separator.set_margin_top(10);
                            separator.set_margin_bottom(10);
                            plugin_ui.append(&separator);

                            // Add a message
                            let message = gtk::Label::new(Some("This UI is provided by the game plugin."));
                            message.set_halign(gtk::Align::Start);
                            plugin_ui.append(&message);

                            plugin_ui.upcast()
                        }
                    } else {
                        // No plugin found for the selected game
                        let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                        error_box.set_margin_top(20);
                        error_box.set_margin_bottom(20);
                        error_box.set_margin_start(20);
                        error_box.set_margin_end(20);

                        let error_label = gtk::Label::new(Some(&format!("No plugin found for game: {}", game_id)));
                        error_label.add_css_class("title-2");
                        error_label.set_halign(gtk::Align::Center);
                        error_box.append(&error_label);

                        error_box.upcast()
                    }
                } else {
                    // No game selected
                    let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                    error_box.set_margin_top(20);
                    error_box.set_margin_bottom(20);
                    error_box.set_margin_start(20);
                    error_box.set_margin_end(20);

                    let error_label = gtk::Label::new(Some("No game selected"));
                    error_label.add_css_class("title-2");
                    error_label.set_halign(gtk::Align::Center);
                    error_box.append(&error_label);

                    error_box.upcast()
                }
            },
            2 => {
                // Profiles view
                if let Some(game_id) = &selected_game_id {
                    if let Some(plugin) = plugin_manager.get_plugin(game_id) {
                        if game_id == "minecraft" {
                            // For Minecraft, use the profiles view from the Minecraft plugin
                            let profiles_view = crate::games::minecraft::ui::ui::build_profiles_view(
                                window.upcast_ref::<gtk::Window>(),
                                toast_overlay,
                                plugin.as_ref(),
                            );

                            profiles_view.upcast()
                        } else {
                            // For other games, use a placeholder
                            let plugin_ui = gtk::Box::new(gtk::Orientation::Vertical, 10);
                            plugin_ui.set_margin_top(20);
                            plugin_ui.set_margin_bottom(20);
                            plugin_ui.set_margin_start(20);
                            plugin_ui.set_margin_end(20);

                            // Add a header
                            let header = gtk::Label::new(Some(&format!("{} - {}", plugin.get_name(), SIDEBAR_ITEMS[row.index() as usize].0)));
                            header.add_css_class("title-2");
                            header.set_halign(gtk::Align::Start);
                            plugin_ui.append(&header);

                            // Add a separator
                            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                            separator.set_margin_top(10);
                            separator.set_margin_bottom(10);
                            plugin_ui.append(&separator);

                            // Add a message
                            let message = gtk::Label::new(Some("This UI is provided by the game plugin."));
                            message.set_halign(gtk::Align::Start);
                            plugin_ui.append(&message);

                            plugin_ui.upcast()
                        }
                    } else {
                        // No plugin found for the selected game
                        let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                        error_box.set_margin_top(20);
                        error_box.set_margin_bottom(20);
                        error_box.set_margin_start(20);
                        error_box.set_margin_end(20);

                        let error_label = gtk::Label::new(Some(&format!("No plugin found for game: {}", game_id)));
                        error_label.add_css_class("title-2");
                        error_label.set_halign(gtk::Align::Center);
                        error_box.append(&error_label);

                        error_box.upcast()
                    }
                } else {
                    // No game selected
                    let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                    error_box.set_margin_top(20);
                    error_box.set_margin_bottom(20);
                    error_box.set_margin_start(20);
                    error_box.set_margin_end(20);

                    let error_label = gtk::Label::new(Some("No game selected"));
                    error_label.add_css_class("title-2");
                    error_label.set_halign(gtk::Align::Center);
                    error_box.append(&error_label);

                    error_box.upcast()
                }
            },
            3 => {
                // Mods view
                if let Some(game_id) = &selected_game_id {
                    if let Some(plugin) = plugin_manager.get_plugin(game_id) {
                        if game_id == "minecraft" {
                            // For Minecraft, use the mods view from the Minecraft plugin
                            let mods_view = crate::games::minecraft::ui::ui::build_mods_view(
                                window.upcast_ref::<gtk::Window>(),
                                toast_overlay,
                                plugin.as_ref(),
                            );

                            mods_view.upcast()
                        } else {
                            // For other games, use a placeholder
                            let plugin_ui = gtk::Box::new(gtk::Orientation::Vertical, 10);
                            plugin_ui.set_margin_top(20);
                            plugin_ui.set_margin_bottom(20);
                            plugin_ui.set_margin_start(20);
                            plugin_ui.set_margin_end(20);

                            // Add a header
                            let header = gtk::Label::new(Some(&format!("{} - {}", plugin.get_name(), SIDEBAR_ITEMS[row.index() as usize].0)));
                            header.add_css_class("title-2");
                            header.set_halign(gtk::Align::Start);
                            plugin_ui.append(&header);

                            // Add a separator
                            let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                            separator.set_margin_top(10);
                            separator.set_margin_bottom(10);
                            plugin_ui.append(&separator);

                            // Add a message
                            let message = gtk::Label::new(Some("This UI is provided by the game plugin."));
                            message.set_halign(gtk::Align::Start);
                            plugin_ui.append(&message);

                            plugin_ui.upcast()
                        }
                    } else {
                        // No plugin found for the selected game
                        let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                        error_box.set_margin_top(20);
                        error_box.set_margin_bottom(20);
                        error_box.set_margin_start(20);
                        error_box.set_margin_end(20);

                        let error_label = gtk::Label::new(Some(&format!("No plugin found for game: {}", game_id)));
                        error_label.add_css_class("title-2");
                        error_label.set_halign(gtk::Align::Center);
                        error_box.append(&error_label);

                        error_box.upcast()
                    }
                } else {
                    // No game selected
                    let error_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                    error_box.set_margin_top(20);
                    error_box.set_margin_bottom(20);
                    error_box.set_margin_start(20);
                    error_box.set_margin_end(20);

                    let error_label = gtk::Label::new(Some("No game selected"));
                    error_label.add_css_class("title-2");
                    error_label.set_halign(gtk::Align::Center);
                    error_box.append(&error_label);

                    error_box.upcast()
                }
            },
            4 => {
                // Simple settings view
                let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
                settings_box.set_margin_top(20);
                settings_box.set_margin_bottom(20);
                settings_box.set_margin_start(20);
                settings_box.set_margin_end(20);

                // Add a header
                let header = gtk::Label::new(Some("Settings"));
                header.add_css_class("title-2");
                header.set_halign(gtk::Align::Start);
                settings_box.append(&header);

                // Add a separator
                let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                separator.set_margin_top(10);
                separator.set_margin_bottom(10);
                settings_box.append(&separator);

                // Add a message
                let message = gtk::Label::new(Some("Settings will be implemented here."));
                message.set_halign(gtk::Align::Start);
                settings_box.append(&message);

                settings_box.upcast()
            },
            _ => {
                warn!("Unknown sidebar selection: {}", row.index());
                gtk::Box::new(gtk::Orientation::Vertical, 0)
            }
        };

        content_container.append(&new_content);
    }
}
