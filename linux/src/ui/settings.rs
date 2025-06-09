use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use log::{info, warn, error, debug};

use crate::config::{Config, save_config};

pub fn build_settings_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
) -> gtk::Box {
    info!("Building settings view");
    // Create a vertical box for the settings view
    let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    settings_box.set_hexpand(true);
    settings_box.set_vexpand(true);
    settings_box.set_size_request(800, 600);

    // Create a header bar
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Settings"))));
    header.set_show_start_title_buttons(false);
    settings_box.append(&header);

    // Create a content box
    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    settings_box.append(&content);

    // Add a preferences group for general settings
    let general_group = adw::PreferencesGroup::new();
    general_group.set_title("General");
    content.append(&general_group);

    // Add a row for the Minecraft directory
    let minecraft_dir_row = adw::ActionRow::new();
    minecraft_dir_row.set_title("Minecraft Directory");

    // Get the selected game's directory
    let config_ref = config.borrow();
    let selected_game_id = config_ref.selected_game.clone().unwrap_or_else(|| {
        if !config_ref.games.is_empty() {
            config_ref.games[0].id.clone()
        } else {
            "minecraft".to_string()
        }
    });

    let game_dir = config_ref.games.iter()
        .find(|g| g.id == selected_game_id)
        .map(|g| g.game_directory.clone())
        .unwrap_or_else(|| PathBuf::from(""));

    minecraft_dir_row.set_subtitle(&game_dir.to_string_lossy());

    let browse_button = gtk::Button::with_label("Browse");
    browse_button.set_valign(gtk::Align::Center);
    minecraft_dir_row.add_suffix(&browse_button);

    general_group.add(&minecraft_dir_row);

    // Connect the browse button
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();
    let minecraft_dir_row_clone = minecraft_dir_row.clone();

    browse_button.connect_clicked(move |_| {
        let file_chooser = gtk::FileDialog::new();
        file_chooser.set_title("Select Minecraft Directory");
        file_chooser.set_modal(true);

        let window_clone = window_clone.clone();
        let config = config_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let minecraft_dir_row = minecraft_dir_row_clone.clone();

        file_chooser.select_folder(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    // Update the config
                    let mut config_mut = config.borrow_mut();
                    let selected_game_id = config_mut.selected_game.clone().unwrap_or_else(|| {
                        if !config_mut.games.is_empty() {
                            config_mut.games[0].id.clone()
                        } else {
                            "minecraft".to_string()
                        }
                    });

                    // Find the selected game and update its directory
                    if let Some(game) = config_mut.games.iter_mut().find(|g| g.id == selected_game_id) {
                        game.game_directory = path.clone();
                    }

                    // Update the UI
                    minecraft_dir_row.set_subtitle(&path.to_string_lossy());

                    // Save the config
                    if let Err(e) = save_config(&config.borrow()) {
                        let toast = adw::Toast::new(&format!("Failed to save config: {}", e));
                        toast_overlay.add_toast(toast);
                    } else {
                        let toast = adw::Toast::new("Minecraft directory updated");
                        toast_overlay.add_toast(toast);
                    }
                }
            }
        });
    });

    // Add a preferences group for advanced settings
    let advanced_group = adw::PreferencesGroup::new();
    advanced_group.set_title("Advanced");
    content.append(&advanced_group);

    // Add a row for Java arguments
    let java_args_row = adw::ActionRow::new();
    java_args_row.set_title("Java Arguments");

    let java_args_entry = gtk::Entry::new();
    let args = &config.borrow().java_arguments;
    if !args.is_empty() {
        java_args_entry.set_text(&args.join(" "));
    }
    java_args_entry.set_placeholder_text(Some("Default"));
    java_args_entry.set_hexpand(true);
    java_args_row.add_suffix(&java_args_entry);

    advanced_group.add(&java_args_row);

    // Connect the Java arguments entry
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();

    java_args_entry.connect_changed(move |entry| {
        let text = entry.text().to_string();
        let mut config_mut = config_clone.borrow_mut();

        if text.is_empty() {
            config_mut.java_arguments = Vec::new();
        } else {
            config_mut.java_arguments = text.split_whitespace().map(|s| s.to_string()).collect();
        }

        // Save the config
        if let Err(e) = save_config(&config_mut) {
            let toast = adw::Toast::new(&format!("Failed to save config: {}", e));
            toast_overlay_clone.add_toast(toast);
        }
    });

    // Add a row for the sandbox mode
    let sandbox_row = adw::ActionRow::new();
    sandbox_row.set_title("Disable Sandbox");
    sandbox_row.set_subtitle("Warning: This may pose a security risk. Only use if you're having issues with authentication.");

    let sandbox_switch = gtk::Switch::new();
    sandbox_switch.set_valign(gtk::Align::Center);
    sandbox_switch.set_active(config.borrow().disable_sandbox);
    sandbox_row.add_suffix(&sandbox_switch);

    advanced_group.add(&sandbox_row);

    // Connect the sandbox switch
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();

    sandbox_switch.connect_active_notify(move |switch| {
        let active = switch.is_active();
        let mut config_mut = config_clone.borrow_mut();
        config_mut.disable_sandbox = active;

        // Save the config
        if let Err(e) = save_config(&config_mut) {
            let toast = adw::Toast::new(&format!("Failed to save config: {}", e));
            toast_overlay_clone.add_toast(toast);
        } else {
            let toast = adw::Toast::new(if active {
                "Sandbox disabled. Please restart the launcher for this to take effect."
            } else {
                "Sandbox enabled. Please restart the launcher for this to take effect."
            });
            toast_overlay_clone.add_toast(toast);
        }
    });

    // Add a preferences group for about
    let about_group = adw::PreferencesGroup::new();
    about_group.set_title("About");
    content.append(&about_group);

    // Add a row for the version
    let version_row = adw::ActionRow::new();
    version_row.set_title("Version");
    version_row.set_subtitle(env!("CARGO_PKG_VERSION"));
    about_group.add(&version_row);

    // Add a row for the GitHub link
    let github_row = adw::ActionRow::new();
    github_row.set_title("GitHub");
    github_row.set_subtitle("View the source code on GitHub");

    let github_button = gtk::Button::with_label("Open");
    github_button.set_valign(gtk::Align::Center);
    github_row.add_suffix(&github_button);

    about_group.add(&github_row);

    // Connect the GitHub button
    let toast_overlay_clone = toast_overlay.clone();

    github_button.connect_clicked(move |_| {
        let result = gtk::gio::AppInfo::launch_default_for_uri(
            "https://github.com/yourusername/mosaic-launcher",
            None::<&gtk::gio::AppLaunchContext>,
        );

        if let Err(e) = result {
            let toast = adw::Toast::new(&format!("Failed to open GitHub: {}", e));
            toast_overlay_clone.add_toast(toast);
        }
    });

    settings_box
}
