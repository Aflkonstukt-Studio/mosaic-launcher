use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use log::{info, warn, error, debug};

use crate::config::{Config, save_config};
use crate::mods::{ModManager, ModSearchParams, ModSearchResult, ModVersionInfo, ModSortField, SortOrder};
use crate::file_manager::FileManager;

pub fn build_mods_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    mod_manager: Rc<RefCell<ModManager>>,
    file_manager: Rc<FileManager>,
) -> gtk::Box {
    info!("Building mods view");
    // Create a vertical box for the mods view
    let mods_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    mods_box.set_hexpand(true);
    mods_box.set_vexpand(true);
    mods_box.set_size_request(800, 600);

    // Create a header bar
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Mods"))));
    header.set_show_start_title_buttons(false);
    mods_box.append(&header);

    // Create a content box
    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    mods_box.append(&content);

    // Add a profile selector
    let profile_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let profile_label = gtk::Label::new(Some("Profile:"));
    profile_box.append(&profile_label);

    let profile_combo = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let profile_model = gtk::StringList::new(&[]);
    profile_combo.set_model(Some(&profile_model));
    profile_combo.set_hexpand(true);
    profile_box.append(&profile_combo);

    content.append(&profile_box);

    // Populate the profile selector
    let profiles = &config.borrow().profiles;
    let mut selected_index = 0;

    if let Some(last_used) = &config.borrow().last_used_profile {
        for (i, profile) in profiles.iter().enumerate() {
            profile_model.append(&profile.name);
            if &profile.id == last_used {
                selected_index = i as u32;
            }
        }
    } else {
        for profile in profiles {
            profile_model.append(&profile.name);
        }
    }

    if !profiles.is_empty() {
        profile_combo.set_selected(selected_index);
    }

    // Add a search box
    let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_hexpand(true);
    search_entry.set_placeholder_text(Some("Search for mods..."));
    search_box.append(&search_entry);

    let search_button = gtk::Button::with_label("Search");
    search_button.add_css_class("suggested-action");
    search_box.append(&search_button);

    content.append(&search_box);

    // Add a scrolled window for the search results
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    content.append(&scrolled);

    // Create a flow box for the search results
    let flow_box = gtk::FlowBox::new();
    flow_box.set_valign(gtk::Align::Start);
    flow_box.set_max_children_per_line(3);
    flow_box.set_selection_mode(gtk::SelectionMode::None);
    flow_box.set_homogeneous(true);
    scrolled.set_child(Some(&flow_box));

    // Connect the search button
    let config_clone = config.clone();
    let mod_manager_clone = mod_manager.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let search_entry_clone = search_entry.clone();
    let profile_combo_clone = profile_combo.clone();
    let flow_box_clone = flow_box.clone();
    let window_clone = window.clone();

    search_button.connect_clicked(move |button| {
        let query = search_entry_clone.text().to_string();
        if query.is_empty() {
            let toast = adw::Toast::new("Please enter a search query");
            toast_overlay_clone.add_toast(toast);
            return;
        }

        let selected = profile_combo_clone.selected();
        if selected == gtk::INVALID_LIST_POSITION {
            let toast = adw::Toast::new("No profile selected");
            toast_overlay_clone.add_toast(toast);
            return;
        }

        let profiles = &config_clone.borrow().profiles;
        if selected as usize >= profiles.len() {
            let toast = adw::Toast::new("Invalid profile selected");
            toast_overlay_clone.add_toast(toast);
            return;
        }

        let profile = &profiles[selected as usize];

        // Show a loading indicator
        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_child(Some(&spinner));
        button.set_sensitive(false);

        // Clear previous results
        while let Some(child) = flow_box_clone.first_child() {
            flow_box_clone.remove(&child);
        }

        // Clone references for the async closure
        let mod_manager = mod_manager_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let button = button.clone();
        let flow_box = flow_box_clone.clone();
        let window = window_clone.clone();
        let config = config_clone.clone();
        let profile_index = selected;

        // Create search parameters
        let params = ModSearchParams {
            query,
            mod_loader: profile.mod_loader.clone(),
            minecraft_version: profile.version.clone(),
            category: None,
            sort_by: ModSortField::Popularity,
            sort_order: SortOrder::Descending,
            limit: 30,
            offset: 0,
        };

        // Perform the search
        gtk::glib::spawn_future_local(async move {
            match mod_manager.borrow().search_mods(&params).await {
                Ok(results) => {
                    if results.is_empty() {
                        let toast = adw::Toast::new("No mods found");
                        toast_overlay.add_toast(toast);
                    } else {
                        for result in results {
                            let mod_card = build_mod_card(
                                &result,
                                &window,
                                &toast_overlay,
                                config.clone(),
                                mod_manager.clone(),
                                profile_index,
                            );
                            flow_box.insert(&mod_card, -1);
                        }
                    }
                }
                Err(e) => {
                    let toast = adw::Toast::new(&format!("Failed to search for mods: {}", e));
                    toast_overlay.add_toast(toast);
                }
            }

            // Reset the button
            button.set_label("Search");
            button.set_sensitive(true);
        });
    });

    mods_box
}

fn build_mod_card(
    mod_result: &ModSearchResult,
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    mod_manager: Rc<RefCell<ModManager>>,
    profile_index: u32,
) -> gtk::Box {
    // Create a card for the mod
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.set_size_request(300, 350);
    card.add_css_class("card");
    card.set_margin_top(10);
    card.set_margin_bottom(10);
    card.set_margin_start(10);
    card.set_margin_end(10);

    // Add the mod icon
    let icon_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    icon_box.set_size_request(300, 150);
    icon_box.set_halign(gtk::Align::Center);
    icon_box.set_valign(gtk::Align::Center);
    icon_box.add_css_class("card-header");

    let icon = if let Some(url) = &mod_result.icon_url {
        // In a real implementation, we would load the icon from the URL
        // For simplicity, we'll just use a placeholder
        gtk::Image::from_icon_name("package-x-generic-symbolic")
    } else {
        gtk::Image::from_icon_name("package-x-generic-symbolic")
    };
    icon.set_pixel_size(64);
    icon_box.append(&icon);

    card.append(&icon_box);

    // Add the mod info
    let info_box = gtk::Box::new(gtk::Orientation::Vertical, 10);
    info_box.set_margin_top(10);
    info_box.set_margin_bottom(10);
    info_box.set_margin_start(10);
    info_box.set_margin_end(10);

    let name_label = gtk::Label::new(Some(&mod_result.name));
    name_label.add_css_class("heading");
    name_label.set_halign(gtk::Align::Start);
    name_label.set_wrap(true);
    name_label.set_max_width_chars(30);
    info_box.append(&name_label);

    let author_label = gtk::Label::new(Some(&format!("by {}", mod_result.author)));
    author_label.set_halign(gtk::Align::Start);
    author_label.add_css_class("caption");
    info_box.append(&author_label);

    let summary_label = gtk::Label::new(Some(&mod_result.summary));
    summary_label.set_halign(gtk::Align::Start);
    summary_label.set_wrap(true);
    summary_label.set_max_width_chars(30);
    summary_label.set_lines(3);
    summary_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    info_box.append(&summary_label);

    let downloads_label = gtk::Label::new(Some(&format!("Downloads: {}", mod_result.download_count)));
    downloads_label.set_halign(gtk::Align::Start);
    downloads_label.add_css_class("caption");
    info_box.append(&downloads_label);

    // Add a spacer
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    info_box.append(&spacer);

    // Add an install button
    let install_button = gtk::Button::with_label("Install");
    install_button.add_css_class("suggested-action");
    install_button.set_halign(gtk::Align::End);
    info_box.append(&install_button);

    card.append(&info_box);

    // Connect the install button
    let mod_result_clone = mod_result.clone();
    let config_clone = config.clone();
    let mod_manager_clone = mod_manager.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();

    install_button.connect_clicked(move |button| {
        // Show a loading indicator
        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_child(Some(&spinner));
        button.set_sensitive(false);

        // Clone references for the async closure
        let mod_result = mod_result_clone.clone();
        let config = config_clone.clone();
        let mod_manager = mod_manager_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let button = button.clone();

        // Check if profile is valid
        {
            let config_ref = config.borrow();
            if profile_index as usize >= config_ref.profiles.len() {
                let toast = adw::Toast::new("Invalid profile selected");
                toast_overlay.add_toast(toast);
                button.set_label("Install");
                button.set_sensitive(true);
                return;
            }
        }

        // Get the latest version
        if mod_result.versions.is_empty() {
            let toast = adw::Toast::new("No versions available for this mod");
            toast_overlay.add_toast(toast);
            button.set_label("Install");
            button.set_sensitive(true);
            return;
        }

        let version = &mod_result.versions[0];
        let version_clone = version.clone();
        let profile_index = profile_index;

        // Get the mod ID for later use
        let mod_id = mod_result.id.clone();

        // Install the mod
        gtk::glib::spawn_future_local(async move {
            // We need to modify our approach to avoid lifetime issues
            // First, extract the profile ID so we can look it up again later
            let profile_id = {
                let config_ref = config.borrow();
                config_ref.profiles[profile_index as usize].id.clone()
            };

            // Now install the mod with a temporary mutable borrow
            let install_result = {
                let mut config_mut = config.borrow_mut();
                let profile = &mut config_mut.profiles[profile_index as usize];

                mod_manager.borrow_mut().install_mod(
                    profile,
                    &mod_result,
                    &version_clone,
                    |progress| {
                        // In a real implementation, we would update a progress bar
                    },
                ).await
            };

            match install_result {
                Ok(_) => {
                    // Update the config with the installed mod
                    {
                        let mut config_mut = config.borrow_mut();
                        let profile = &mut config_mut.profiles[profile_index as usize];
                        // Update profile with installed mod if needed
                        // For example: profile.mods.push(mod_result.id.clone());
                    }

                    let toast = adw::Toast::new(&format!("Installed {} successfully", mod_result.name));
                    toast_overlay.add_toast(toast);

                    // Save the config
                    if let Err(e) = save_config(&config.borrow()) {
                        let toast = adw::Toast::new(&format!("Failed to save config: {}", e));
                        toast_overlay.add_toast(toast);
                    }
                }
                Err(e) => {
                    let toast = adw::Toast::new(&format!("Failed to install mod: {}", e));
                    toast_overlay.add_toast(toast);
                }
            }

            // Reset the button
            button.set_label("Install");
            button.set_sensitive(true);
        });
    });

    card
}
