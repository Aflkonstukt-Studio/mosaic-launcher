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
    let profiles = &config.borrow().profiles;
    for profile in profiles {
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

    // Connect the save button
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let list_box_clone = list_box.clone();
    let name_entry_clone = name_entry.clone();
    let version_row_clone = version_row.clone();
    let loader_row_clone = loader_row.clone();
    let game_dir_entry_clone = game_dir_entry.clone();
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
        let profile_id = if let Some(profile) = &profile {
            // Update existing profile
            for p in &mut config_mut.profiles {
                if p.id == profile.id {
                    p.name = name;
                    p.version = version;
                    p.mod_loader = Some(mod_loader);
                    p.game_directory = if game_dir.is_empty() {
                        None
                    } else {
                        Some(PathBuf::from(game_dir))
                    };
                    break;
                }
            }
            profile.id.clone()
        } else {
            // Create a new profile
            let id = Uuid::new_v4().to_string();
            config_mut.profiles.push(Profile {
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

        for profile in &config_mut.profiles {
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
