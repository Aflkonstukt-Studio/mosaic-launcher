use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::config::{Config, Profile, ModLoader, save_config};
use crate::minecraft::VersionManifest;
use crate::file_manager::FileManager;
use std::fs;

pub fn build_profiles_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
) -> gtk::Box {
    info!("Building profiles view");
    // Create a vertical box for the profiles view
    let profiles_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    profiles_box.set_hexpand(true);
    profiles_box.set_vexpand(true);
    profiles_box.set_size_request(800, 600);

    // Create a header bar
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Profiles"))));
    header.set_show_start_title_buttons(false);

    // Add a new profile button
    let new_profile_button = gtk::Button::new();
    new_profile_button.set_icon_name("list-add-symbolic");
    new_profile_button.set_tooltip_text(Some("Create new profile"));
    header.pack_end(&new_profile_button);

    profiles_box.append(&header);

    // Create a scrolled window for the profiles list
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    profiles_box.append(&scrolled);

    // Create a list box for the profiles
    let list_box = gtk::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_selection_mode(gtk::SelectionMode::None);
    list_box.set_margin_top(20);
    list_box.set_margin_bottom(20);
    list_box.set_margin_start(20);
    list_box.set_margin_end(20);
    scrolled.set_child(Some(&list_box));

    // Populate the profiles list
    let config_ref = config.borrow();
    let selected_game_id = config_ref.selected_game.clone().unwrap_or_else(|| {
        if !config_ref.games.is_empty() {
            config_ref.games[0].id.clone()
        } else {
            "minecraft".to_string()
        }
    });

    let game = config_ref.games.iter()
        .find(|g| g.id == selected_game_id)
        .unwrap_or_else(|| {
            // If the selected game doesn't exist, use the first game
            if !config_ref.games.is_empty() {
                &config_ref.games[0]
            } else {
                panic!("No games found in config")
            }
        });

    let profiles = &game.profiles;
    for profile in profiles {
        let row = adw::ActionRow::new();
        row.set_title(&profile.name);
        row.set_subtitle(&format!("Minecraft {}", profile.version));

        // Add an edit button
        let edit_button = gtk::Button::new();
        edit_button.set_icon_name("document-edit-symbolic");
        edit_button.set_tooltip_text(Some("Edit profile"));
        edit_button.add_css_class("flat");

        // Connect the edit button
        let config_clone = config.clone();
        let version_manifest_clone = version_manifest.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let window_clone = window.clone();
        let list_box_clone = list_box.clone();
        let profile_clone = profile.clone();

        edit_button.connect_clicked(move |_| {
            show_profile_dialog(
                &window_clone,
                &toast_overlay_clone,
                config_clone.clone(),
                version_manifest_clone.clone(),
                Some(profile_clone.clone()),
                list_box_clone.clone(),
            );
        });

        row.add_suffix(&edit_button);

        // Add a delete button
        let delete_button = gtk::Button::new();
        delete_button.set_icon_name("user-trash-symbolic");
        delete_button.set_tooltip_text(Some("Delete profile"));
        delete_button.add_css_class("flat");

        // Connect the delete button
        let config_clone = config.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let list_box_clone = list_box.clone();
        let profile_id = profile.id.clone();
        let profile_name = profile.name.clone();
        let window_clone = window.clone();

        delete_button.connect_clicked(move |_| {
            // Create a confirmation dialog
            let dialog = gtk::MessageDialog::new(
                Some(&window_clone),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Question,
                gtk::ButtonsType::None,
                &format!("Are you sure you want to delete the profile '{}'? This will also delete all of its files and libraries.", profile_name),
            );
            dialog.set_title(Some(&format!("Delete Profile '{}'", profile_name)));

            dialog.add_button("Cancel", gtk::ResponseType::Cancel);
            dialog.add_button("Delete", gtk::ResponseType::Accept);
            dialog.set_default_response(gtk::ResponseType::Cancel);

            // Clone the variables before using them in the inner closure
            let config_clone2 = config_clone.clone();
            let profile_id2 = profile_id.clone();
            let toast_overlay_clone2 = toast_overlay_clone.clone();
            let list_box_clone2 = list_box_clone.clone();

            dialog.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    // Delete the profile
                    let mut config_mut = config_clone2.borrow_mut();

                    // Get the selected game ID
                    let selected_game_id = config_mut.selected_game.clone().unwrap_or_else(|| {
                        if !config_mut.games.is_empty() {
                            config_mut.games[0].id.clone()
                        } else {
                            "minecraft".to_string()
                        }
                    });

                    // Find the game index
                    let game_index = if let Some(index) = config_mut.games.iter().position(|g| g.id == selected_game_id) {
                        index
                    } else if !config_mut.games.is_empty() {
                        0
                    } else {
                        panic!("No games found in config")
                    };

                    // Get a mutable reference to the game
                    let game = &mut config_mut.games[game_index];

                    // Find the profile index
                    if let Some(profile_index) = game.profiles.iter().position(|p| p.id == profile_id2) {
                        // Get the profile's game directory
                        let game_dir = if let Some(dir) = &game.profiles[profile_index].game_directory {
                            Some(dir.clone())
                        } else {
                            None
                        };

                        // Remove the profile from the config
                        game.profiles.remove(profile_index);

                        // Save the config
                        if let Err(e) = save_config(&config_mut) {
                            let toast = adw::Toast::new(&format!("Failed to delete profile: {}", e));
                            toast_overlay_clone2.add_toast(toast);
                        } else {
                            // Delete the profile's files and libraries if it has a custom game directory
                            if let Some(dir) = game_dir {
                                // Delete the directory asynchronously
                                let file_manager = FileManager::new();
                                let toast_overlay_clone3 = toast_overlay_clone2.clone();

                                glib::MainContext::default().spawn_local(async move {
                                    match file_manager.remove_dir_all(&dir).await {
                                        Ok(_) => {
                                            let toast = adw::Toast::new("Profile and its files deleted successfully");
                                            toast_overlay_clone3.add_toast(toast);
                                        },
                                        Err(e) => {
                                            let toast = adw::Toast::new(&format!("Failed to delete profile files: {}", e));
                                            toast_overlay_clone3.add_toast(toast);
                                        }
                                    }
                                });
                            } else {
                                let toast = adw::Toast::new("Profile deleted successfully");
                                toast_overlay_clone2.add_toast(toast);
                            }

                            // Refresh the profiles list
                            while let Some(child) = list_box_clone2.first_child() {
                                list_box_clone2.remove(&child);
                            }

                            // Get the game's profiles
                            let game = &config_mut.games[game_index];
                            for profile in &game.profiles {
                                let row = adw::ActionRow::new();
                                row.set_title(&profile.name);
                                row.set_subtitle(&format!("Minecraft {}", profile.version));

                                // Add an edit button
                                let edit_button = gtk::Button::new();
                                edit_button.set_icon_name("document-edit-symbolic");
                                edit_button.set_tooltip_text(Some("Edit profile"));
                                edit_button.add_css_class("flat");
                                row.add_suffix(&edit_button);

                                // Add a delete button
                                let delete_button = gtk::Button::new();
                                delete_button.set_icon_name("user-trash-symbolic");
                                delete_button.set_tooltip_text(Some("Delete profile"));
                                delete_button.add_css_class("flat");
                                row.add_suffix(&delete_button);

                                list_box_clone2.append(&row);
                            }
                        }
                    }
                }

                dialog.destroy();
            });

            dialog.present();
        });

        row.add_suffix(&delete_button);

        list_box.append(&row);
    }

    // If there are no profiles, show a message
    if profiles.is_empty() {
        let placeholder = gtk::Box::new(gtk::Orientation::Vertical, 10);
        placeholder.set_halign(gtk::Align::Center);
        placeholder.set_valign(gtk::Align::Center);

        let label = gtk::Label::new(Some("No profiles yet"));
        label.add_css_class("title-2");
        placeholder.append(&label);

        let subtitle = gtk::Label::new(Some("Click the + button to create a new profile"));
        subtitle.add_css_class("title-4");
        placeholder.append(&subtitle);

        scrolled.set_child(Some(&placeholder));
    }

    // Connect the new profile button
    let config_clone = config.clone();
    let version_manifest_clone = version_manifest.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();
    let list_box_clone = list_box.clone();

    new_profile_button.connect_clicked(move |_| {
        show_profile_dialog(
            &window_clone,
            &toast_overlay_clone,
            config_clone.clone(),
            version_manifest_clone.clone(),
            None,
            list_box_clone.clone(),
        );
    });

    info!("Built profiles view");   

    profiles_box
}

fn show_profile_dialog(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
    profile: Option<Profile>,
    list_box: gtk::ListBox,
) {
    // Create a dialog for the profile
    let dialog = adw::PreferencesWindow::new();
    dialog.set_transient_for(Some(window));
    dialog.set_modal(true);
    dialog.set_title(Some(if profile.is_some() { "Edit Profile" } else { "Create Profile" }));
    dialog.set_default_width(500);
    dialog.set_default_height(600);

    // We'll add a create button later after we've set up all the form fields

    // Create a page for general settings
    let general_page = adw::PreferencesPage::new();
    general_page.set_title("General");
    general_page.set_icon_name(Some("preferences-system-symbolic"));
    dialog.add(&general_page);

    // Create a group for basic settings
    let basic_group = adw::PreferencesGroup::new();
    basic_group.set_title("Basic Settings");
    general_page.add(&basic_group);

    // Add a row for the profile name
    let name_row = adw::ActionRow::new();
    name_row.set_title("Profile Name");

    let name_entry = gtk::Entry::new();
    if let Some(profile) = &profile {
        name_entry.set_text(&profile.name);
    } else {
        name_entry.set_text("New Profile");
    }
    name_entry.set_hexpand(true);
    name_row.add_suffix(&name_entry);

    basic_group.add(&name_row);

    // Add a row for the Minecraft version
    let version_row = adw::ComboRow::new();
    version_row.set_title("Minecraft Version");
    let version_model = gtk::StringList::new(&[]);
    version_row.set_model(Some(&version_model));

    // Populate the version dropdown
    if let Some(manifest) = &*version_manifest.lock().unwrap() {
        // Add all versions to the dropdown
        for version in &manifest.versions {
            version_model.append(&version.id);
        }

        // Select the appropriate version
        if let Some(profile) = &profile {
            // For existing profiles, select the current version
            for (i, version) in manifest.versions.iter().enumerate() {
                if version.id == profile.version {
                    version_row.set_selected(i as u32);
                    break;
                }
            }
        } else {
            // For new profiles, select the latest release
            let latest_release = &manifest.latest.release;
            for (i, version) in manifest.versions.iter().enumerate() {
                if &version.id == latest_release {
                    version_row.set_selected(i as u32);
                    break;
                }
            }
        }
    } else {
        // If the version manifest isn't loaded, show a placeholder
        version_model.append("Loading...");
        version_row.set_selected(0);
        version_row.set_sensitive(false);
    }
    basic_group.add(&version_row);

    // Add a row for the mod loader
    let loader_row = adw::ComboRow::new();
    loader_row.set_title("Mod Loader");
    let loader_model = gtk::StringList::new(&["None", "Forge", "Fabric", "Quilt"]);
    loader_row.set_model(Some(&loader_model));
    if let Some(profile) = &profile {
        match &profile.mod_loader {
            Some(ModLoader::Forge) => loader_row.set_selected(1),
            Some(ModLoader::Fabric) => loader_row.set_selected(2),
            Some(ModLoader::Quilt) => loader_row.set_selected(3),
            Some(ModLoader::None) | None => loader_row.set_selected(0),
        }
    } else {
        // Default to None for new profiles
        loader_row.set_selected(0);
    }
    basic_group.add(&loader_row);

    // Add a row for the game directory
    let game_dir_row = adw::ActionRow::new();
    game_dir_row.set_title("Game Directory");
    let game_dir_entry = gtk::Entry::new();
    if let Some(profile) = &profile {
        if let Some(game_dir) = &profile.game_directory {
            game_dir_entry.set_text(&game_dir.to_string_lossy());
        }
    }
    game_dir_entry.set_placeholder_text(Some("Default"));
    game_dir_entry.set_hexpand(true);
    game_dir_row.add_suffix(&game_dir_entry);
    basic_group.add(&game_dir_row);

    // Add a browse button for the game directory
    let browse_button = gtk::Button::with_label("Browse");
    browse_button.set_valign(gtk::Align::Center);
    game_dir_row.add_suffix(&browse_button);

    // Add a row for RAM allocation
    let memory_row = adw::ActionRow::new();
    memory_row.set_title("RAM Allocation (MB)");
    let memory_entry = gtk::SpinButton::with_range(512.0, 32768.0, 512.0);

    // Set the default value from the profile or global config
    let config_ref = config.borrow();
    let default_memory = if let Some(profile) = &profile {
        profile.memory.unwrap_or(config_ref.max_memory)
    } else {
        config_ref.max_memory
    };
    memory_entry.set_value(default_memory as f64);

    memory_row.add_suffix(&memory_entry);
    basic_group.add(&memory_row);

    // Connect the browse button
    let window_clone = window.clone();
    let game_dir_entry_clone = game_dir_entry.clone();
    browse_button.connect_clicked(move |_| {
        let file_chooser = gtk::FileDialog::new();
        file_chooser.set_title("Select Game Directory");
        file_chooser.set_modal(true);

        let window_clone = window_clone.clone();
        let game_dir_entry = game_dir_entry_clone.clone();
        file_chooser.select_folder(Some(&window_clone), None::<&gtk::gio::Cancellable>, move |result| {
            if let Ok(file) = result {
                if let Some(path) = file.path() {
                    game_dir_entry.set_text(&path.to_string_lossy());
                }
            }
        });
    });

    // Add a create button for new profiles
    if profile.is_none() {
        // Create a button group
        let button_group = adw::PreferencesGroup::new();
        general_page.add(&button_group);

        // Create the button
        let create_button = gtk::Button::new();
        create_button.set_label("Create Profile");
        create_button.add_css_class("suggested-action");
        create_button.set_margin_top(10);
        create_button.set_margin_bottom(10);
        create_button.set_margin_start(10);
        create_button.set_margin_end(10);
        create_button.set_halign(gtk::Align::End);

        // Create a cancel button
        let cancel_button = gtk::Button::new();
        cancel_button.set_label("Cancel");
        cancel_button.set_margin_top(10);
        cancel_button.set_margin_bottom(10);
        cancel_button.set_margin_start(10);
        cancel_button.set_margin_end(10);
        cancel_button.set_halign(gtk::Align::End);

        // Add the buttons to a box
        let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        button_box.set_hexpand(true);
        button_box.set_halign(gtk::Align::End);
        button_box.append(&cancel_button);
        button_box.append(&create_button);

        // Add the box to the group
        button_group.add(&button_box);

        // Connect the cancel button
        let dialog_clone2 = dialog.clone();
        cancel_button.connect_clicked(move |_| {
            dialog_clone2.close();
        });

        // Connect the create button
        let config_clone = config.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let list_box_clone = list_box.clone();
        let name_entry_clone = name_entry.clone();
        let version_row_clone = version_row.clone();
        let loader_row_clone = loader_row.clone();
        let game_dir_entry_clone = game_dir_entry.clone();
        let memory_entry_clone = memory_entry.clone();
        let version_manifest_clone = version_manifest.clone();
        let dialog_clone = dialog.clone();

        create_button.connect_clicked(move |_| {
            // Get the values from the form
            let name = name_entry_clone.text().to_string();
            let version_index = version_row_clone.selected();
            let loader_index = loader_row_clone.selected();
            let game_dir = game_dir_entry_clone.text().to_string();
            let memory = memory_entry_clone.value() as u32;

            // Validate the form
            if name.is_empty() {
                let toast = adw::Toast::new("Profile name cannot be empty");
                toast_overlay_clone.add_toast(toast);
                return;
            }

            if version_index == gtk::INVALID_LIST_POSITION {
                let toast = adw::Toast::new("Please select a Minecraft version");
                toast_overlay_clone.add_toast(toast);
                return;
            }

            // Get the selected version
            let version = if let Some(manifest) = &*version_manifest_clone.lock().unwrap() {
                if version_index as usize >= manifest.versions.len() {
                    let toast = adw::Toast::new("Invalid Minecraft version selected");
                    toast_overlay_clone.add_toast(toast);
                    return;
                }
                manifest.versions[version_index as usize].id.clone()
            } else {
                let toast = adw::Toast::new("Failed to load Minecraft versions");
                toast_overlay_clone.add_toast(toast);
                return;
            };

            // Get the selected mod loader
            let mod_loader = match loader_index {
                0 => ModLoader::None,
                1 => ModLoader::Forge,
                2 => ModLoader::Fabric,
                3 => ModLoader::Quilt,
                _ => {
                    let toast = adw::Toast::new("Invalid mod loader selected");
                    toast_overlay_clone.add_toast(toast);
                    return;
                }
            };

            // Create the profile
            let mut config_mut = config_clone.borrow_mut();

            // Get the selected game ID
            let selected_game_id = config_mut.selected_game.clone().unwrap_or_else(|| {
                if !config_mut.games.is_empty() {
                    config_mut.games[0].id.clone()
                } else {
                    "minecraft".to_string()
                }
            });

            // Find the game index
            let game_index = if let Some(index) = config_mut.games.iter().position(|g| g.id == selected_game_id) {
                index
            } else if !config_mut.games.is_empty() {
                0
            } else {
                panic!("No games found in config")
            };

            // Get a mutable reference to the game
            let game = &mut config_mut.games[game_index];

            // Create a new profile
            let id = Uuid::new_v4().to_string();
            game.profiles.push(Profile {
                id: id.clone(),
                name,
                version,
                mod_loader: Some(mod_loader),
                mod_loader_version: None,
                game_directory: if game_dir.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(game_dir))
                },
                resolution: None,
                mods: Vec::new(),
                memory: Some(memory),
            });

            // Save the config
            if let Err(e) = save_config(&config_mut) {
                let toast = adw::Toast::new(&format!("Failed to save profile: {}", e));
                toast_overlay_clone.add_toast(toast);
            } else {
                let toast = adw::Toast::new("Profile created successfully");
                toast_overlay_clone.add_toast(toast);
            }

            // Refresh the profiles list
            while let Some(child) = list_box_clone.first_child() {
                list_box_clone.remove(&child);
            }

            // Get the game's profiles
            let game = &config_mut.games[game_index];
            for profile in &game.profiles {
                let row = adw::ActionRow::new();
                row.set_title(&profile.name);
                row.set_subtitle(&format!("Minecraft {}", profile.version));

                // Add an edit button
                let edit_button = gtk::Button::new();
                edit_button.set_icon_name("document-edit-symbolic");
                edit_button.set_tooltip_text(Some("Edit profile"));
                edit_button.add_css_class("flat");
                row.add_suffix(&edit_button);

                // Add a delete button
                let delete_button = gtk::Button::new();
                delete_button.set_icon_name("user-trash-symbolic");
                delete_button.set_tooltip_text(Some("Delete profile"));
                delete_button.add_css_class("flat");
                row.add_suffix(&delete_button);

                list_box_clone.append(&row);
            }

            // Close the dialog
            dialog_clone.close();
        });

        // For new profiles, don't save when the user clicks the X button
        dialog.connect_close_request(move |dialog| {
            dialog.close();
            glib::Propagation::Proceed
        });

        // Present the dialog
        dialog.present();
    } else {
        // For existing profiles, save when the user clicks the X button
        let config_clone = config.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let list_box_clone = list_box.clone();
        let name_entry_clone = name_entry.clone();
        let version_row_clone = version_row.clone();
        let loader_row_clone = loader_row.clone();
        let game_dir_entry_clone = game_dir_entry.clone();
        let memory_entry_clone = memory_entry.clone();
        let version_manifest_clone = version_manifest.clone();

        dialog.connect_close_request(move |dialog| {
            // Get the values from the form
            let name = name_entry_clone.text().to_string();
            let version_index = version_row_clone.selected();
            let loader_index = loader_row_clone.selected();
            let game_dir = game_dir_entry_clone.text().to_string();

            // Validate the form
            if name.is_empty() {
                let toast = adw::Toast::new("Profile name cannot be empty");
                toast_overlay_clone.add_toast(toast);
                return glib::Propagation::Stop;
            }

            if version_index == gtk::INVALID_LIST_POSITION {
                let toast = adw::Toast::new("Please select a Minecraft version");
                toast_overlay_clone.add_toast(toast);
                return glib::Propagation::Stop;
            }

            // Get the selected version
            let version = if let Some(manifest) = &*version_manifest_clone.lock().unwrap() {
                if version_index as usize >= manifest.versions.len() {
                    let toast = adw::Toast::new("Invalid Minecraft version selected");
                    toast_overlay_clone.add_toast(toast);
                    return glib::Propagation::Stop;
                }
                manifest.versions[version_index as usize].id.clone()
            } else {
                let toast = adw::Toast::new("Failed to load Minecraft versions");
                toast_overlay_clone.add_toast(toast);
                return glib::Propagation::Stop;
            };

            // Get the selected mod loader
            let mod_loader = match loader_index {
                0 => ModLoader::None,
                1 => ModLoader::Forge,
                2 => ModLoader::Fabric,
                3 => ModLoader::Quilt,
                _ => {
                    let toast = adw::Toast::new("Invalid mod loader selected");
                    toast_overlay_clone.add_toast(toast);
                    return glib::Propagation::Stop;
                }
            };

            // Create or update the profile
            let mut config_mut = config_clone.borrow_mut();

            // Get the selected game ID
            let selected_game_id = config_mut.selected_game.clone().unwrap_or_else(|| {
                if !config_mut.games.is_empty() {
                    config_mut.games[0].id.clone()
                } else {
                    "minecraft".to_string()
                }
            });

            // Find the game index
            let game_index = if let Some(index) = config_mut.games.iter().position(|g| g.id == selected_game_id) {
                index
            } else if !config_mut.games.is_empty() {
                0
            } else {
                panic!("No games found in config")
            };

            // Get a mutable reference to the game
            let game = &mut config_mut.games[game_index];

            let profile_id = if let Some(profile) = &profile {
                // Update existing profile
                for p in &mut game.profiles {
                    if p.id == profile.id {
                        p.name = name;
                        p.version = version;
                        p.mod_loader = Some(mod_loader);
                        p.game_directory = if game_dir.is_empty() {
                            None
                        } else {
                            Some(PathBuf::from(game_dir))
                        };
                        p.memory = Some(memory_entry.value() as u32);
                        break;
                    }
                }
                profile.id.clone()
            } else {
                // Create a new profile
                let id = Uuid::new_v4().to_string();
                game.profiles.push(Profile {
                    id: id.clone(),
                    name,
                    version,
                    mod_loader: Some(mod_loader),
                    mod_loader_version: None,
                    game_directory: if game_dir.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(game_dir))
                    },
                    resolution: None,
                    mods: Vec::new(),
                    memory: Some(memory_entry.value() as u32),
                });
                id
            };

            // Save the config
            if let Err(e) = save_config(&config_mut) {
                let toast = adw::Toast::new(&format!("Failed to save profile: {}", e));
                toast_overlay_clone.add_toast(toast);
            } else {
                let toast = adw::Toast::new(if profile.is_some() {
                    "Profile updated successfully"
                } else {
                    "Profile created successfully"
                });
                toast_overlay_clone.add_toast(toast);
            }

            // Refresh the profiles list
            while let Some(child) = list_box_clone.first_child() {
                list_box_clone.remove(&child);
            }

            // Get the game's profiles
            let game = &config_mut.games[game_index];
            for profile in &game.profiles {
                let row = adw::ActionRow::new();
                row.set_title(&profile.name);
                row.set_subtitle(&format!("Minecraft {}", profile.version));

                // Add an edit button
                let edit_button = gtk::Button::new();
                edit_button.set_icon_name("document-edit-symbolic");
                edit_button.set_tooltip_text(Some("Edit profile"));
                edit_button.add_css_class("flat");
                row.add_suffix(&edit_button);

                // Add a delete button
                let delete_button = gtk::Button::new();
                delete_button.set_icon_name("user-trash-symbolic");
                delete_button.set_tooltip_text(Some("Delete profile"));
                delete_button.add_css_class("flat");
                row.add_suffix(&delete_button);

                list_box_clone.append(&row);
            }

            dialog.close();
            glib::Propagation::Proceed
        });

        dialog.present();
    }
}
