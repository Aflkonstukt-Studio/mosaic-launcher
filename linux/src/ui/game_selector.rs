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

use crate::config::{Config, Game, GameType, save_config};

pub fn build_game_selector(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
) -> gtk::Box {
    // Create a vertical box for the game selector
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

    // Create a scrolled window for the game list
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_vexpand(true);
    scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    // Create a list box for the games
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.add_css_class("boxed-list");
    scrolled_window.set_child(Some(&list_box));
    game_selector_box.append(&scrolled_window);

    // Populate the list box with games from the config
    let config_clone = config.clone();
    let selected_game_id = config.borrow().selected_game.clone();

    for (index, game) in config.borrow().games.iter().enumerate() {
        // Create a row for the game
        let row = adw::ActionRow::new();
        row.set_title(&game.name);

        // Set subtitle based on game type
        match game.game_type {
            GameType::Minecraft => row.set_subtitle("Minecraft"),
            GameType::Custom => {
                if let Some(executable) = &game.executable {
                    row.set_subtitle(&format!("Custom Game: {}", executable.to_string_lossy()));
                } else {
                    row.set_subtitle("Custom Game");
                }
            }
        }

        // Add an icon
        let icon_name = match game.game_type {
            GameType::Minecraft => "applications-games-symbolic",
            GameType::Custom => "application-x-executable-symbolic",
        };
        let icon = gtk::Image::from_icon_name(icon_name);
        row.add_prefix(&icon);

        // Add a select button
        let select_button = gtk::Button::new();
        select_button.set_icon_name("go-next-symbolic");
        select_button.set_tooltip_text(Some("Select this game"));
        select_button.add_css_class("flat");
        row.add_suffix(&select_button);

        // Store the game ID in the row
        unsafe { row.set_data("game-id", game.id.clone()); }

        // Connect the select button click event
        let game_id = game.id.clone();
        let config_clone = config_clone.clone();
        let toast_overlay_clone = toast_overlay.clone();
        select_button.connect_clicked(move |_| {
            // Update the selected game in the config
            let mut config = config_clone.borrow_mut();
            config.selected_game = Some(game_id.clone());

            // Save the config
            if let Err(e) = save_config(&config) {
                error!("Failed to save config: {}", e);
                let toast = adw::Toast::new(&format!("Failed to save game selection: {}", e));
                toast_overlay_clone.add_toast(toast);
            } else {
                info!("Selected game: {}", game_id);
                let toast = adw::Toast::new(&format!("Selected game: {}", game_id));
                toast_overlay_clone.add_toast(toast);
            }
        });

        // Add the row to the list box
        list_box.append(&row);

        // Select the row if it's the currently selected game
        if let Some(selected_id) = &selected_game_id {
            if selected_id == &game.id {
                list_box.select_row(Some(&row));
            }
        } else if index == 0 {
            // If no game is selected, select the first one
            list_box.select_row(Some(&row));
        }
    }

    // Note: We don't connect a row-selected handler here
    // The app.rs file will handle row selection and navigation

    // Add a button to add a new game
    let add_button = gtk::Button::with_label("Add Custom Game");
    add_button.add_css_class("suggested-action");
    add_button.set_halign(gtk::Align::Center);
    add_button.set_margin_top(10);

    // Clone references for the closure
    let window_clone = window.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let config_clone = config.clone();

    // Connect the add button click event
    add_button.connect_clicked(move |_| {
        // Create a dialog for adding a new game
        let dialog = gtk::Dialog::new();
        dialog.set_title(Some("Add Custom Game"));
        dialog.set_modal(true);
        dialog.set_transient_for(Some(&window_clone));
        dialog.set_default_width(400);

        // Create a header bar with title
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_title_widget(Some(&gtk::Label::builder()
            .label("Add Custom Game")
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

        // Add name field
        let name_label = gtk::Label::new(Some("Game Name:"));
        name_label.set_halign(gtk::Align::Start);
        content_area.append(&name_label);

        let name_entry = gtk::Entry::new();
        name_entry.set_placeholder_text(Some("Enter game name"));
        content_area.append(&name_entry);

        // Add executable field
        let executable_label = gtk::Label::new(Some("Executable Path:"));
        executable_label.set_halign(gtk::Align::Start);
        content_area.append(&executable_label);

        let executable_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let executable_entry = gtk::Entry::new();
        executable_entry.set_placeholder_text(Some("Path to game executable"));
        executable_entry.set_hexpand(true);
        executable_box.append(&executable_entry);

        let browse_button = gtk::Button::with_label("Browse");
        executable_box.append(&browse_button);
        content_area.append(&executable_box);

        // Connect browse button
        let window_for_browse = window_clone.clone();
        let executable_entry_clone = executable_entry.clone();
        browse_button.connect_clicked(move |_| {
            let file_chooser = gtk::FileChooserDialog::new(
                Some("Select Game Executable"),
                Some(&window_for_browse),
                gtk::FileChooserAction::Open,
                &[("Cancel", gtk::ResponseType::Cancel), ("Open", gtk::ResponseType::Accept)]
            );

            file_chooser.connect_response(clone!(@weak executable_entry_clone => move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            executable_entry_clone.set_text(&path.to_string_lossy());
                        }
                    }
                }
                dialog.destroy();
            }));

            file_chooser.show();
        });

        // Add game directory field
        let directory_label = gtk::Label::new(Some("Game Directory:"));
        directory_label.set_halign(gtk::Align::Start);
        content_area.append(&directory_label);

        let directory_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let directory_entry = gtk::Entry::new();
        directory_entry.set_placeholder_text(Some("Path to game directory"));
        directory_entry.set_hexpand(true);
        directory_box.append(&directory_entry);

        let browse_dir_button = gtk::Button::with_label("Browse");
        directory_box.append(&browse_dir_button);
        content_area.append(&directory_box);

        // Connect browse directory button
        let window_for_browse_dir = window_clone.clone();
        let directory_entry_clone = directory_entry.clone();
        browse_dir_button.connect_clicked(move |_| {
            let file_chooser = gtk::FileChooserDialog::new(
                Some("Select Game Directory"),
                Some(&window_for_browse_dir),
                gtk::FileChooserAction::SelectFolder,
                &[("Cancel", gtk::ResponseType::Cancel), ("Open", gtk::ResponseType::Accept)]
            );

            file_chooser.connect_response(clone!(@weak directory_entry_clone => move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            directory_entry_clone.set_text(&path.to_string_lossy());
                        }
                    }
                }
                dialog.destroy();
            }));

            file_chooser.show();
        });

        // Add action buttons
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        dialog.add_button("Add Game", gtk::ResponseType::Accept);
        dialog.set_default_response(gtk::ResponseType::Accept);

        // Clone references for the response handler
        let name_entry_clone = name_entry.clone();
        let executable_entry_clone = executable_entry.clone();
        let directory_entry_clone = directory_entry.clone();
        let config_clone_for_response = config_clone.clone();
        let toast_overlay_clone_for_response = toast_overlay_clone.clone();

        // Connect the response signal
        dialog.connect_response(move |dialog, response| {
            dialog.destroy();

            if response == gtk::ResponseType::Accept {
                let name = name_entry_clone.text().to_string();
                let executable = executable_entry_clone.text().to_string();
                let directory = directory_entry_clone.text().to_string();

                // Validate inputs
                if name.is_empty() {
                    let toast = adw::Toast::new("Game name cannot be empty");
                    toast_overlay_clone_for_response.add_toast(toast);
                    return;
                }

                if directory.is_empty() {
                    let toast = adw::Toast::new("Game directory cannot be empty");
                    toast_overlay_clone_for_response.add_toast(toast);
                    return;
                }

                // Create a new game
                let game = Game {
                    id: format!("custom_{}", glib::uuid_string_random()),
                    name,
                    icon: None,
                    executable: if executable.is_empty() { None } else { Some(std::path::PathBuf::from(executable)) },
                    game_directory: std::path::PathBuf::from(directory),
                    profiles: vec![],
                    game_type: GameType::Custom,
                };

                // Add the game to the config
                let mut config = config_clone_for_response.borrow_mut();
                config.games.push(game.clone());
                config.selected_game = Some(game.id.clone());

                // Save the config
                if let Err(e) = save_config(&config) {
                    error!("Failed to save config: {}", e);
                    let toast = adw::Toast::new(&format!("Failed to add game: {}", e));
                    toast_overlay_clone_for_response.add_toast(toast);
                } else {
                    info!("Added new game: {}", game.name);
                    let toast = adw::Toast::new(&format!("Added new game: {}", game.name));
                    toast_overlay_clone_for_response.add_toast(toast);

                    // Reload the game selector
                    // In a real implementation, we would update the list box directly
                    // For simplicity, we'll just show a message asking the user to restart
                    let restart_toast = adw::Toast::new("Please restart the launcher to see the new game");
                    toast_overlay_clone_for_response.add_toast(restart_toast);
                }
            }
        });

        // Show the dialog
        dialog.present();
    });

    game_selector_box.append(&add_button);

    game_selector_box
}
