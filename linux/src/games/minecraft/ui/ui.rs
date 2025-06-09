// Minecraft UI components for Mosaic Launcher

use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::sync::{Arc, Mutex};
use crate::games::minecraft::auth::AuthSession;
use crate::games::GamePlugin;

/// Build the play view for Minecraft
pub fn build_play_view(
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    plugin: &dyn GamePlugin,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
) -> gtk::Box {
    // Create a vertical box for the play view
    let play_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    play_box.set_margin_top(20);
    play_box.set_margin_bottom(20);
    play_box.set_margin_start(20);
    play_box.set_margin_end(20);

    // Add a header
    let header = gtk::Label::new(Some("Play Minecraft"));
    header.add_css_class("title-2");
    header.set_halign(gtk::Align::Start);
    play_box.append(&header);

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(10);
    separator.set_margin_bottom(10);
    play_box.append(&separator);

    // Get profiles from the plugin
    let profiles = plugin.get_profiles();

    // Add a profile selector
    let profile_combo = gtk::ComboBoxText::new();
    for profile in &profiles {
        profile_combo.append(Some(&profile.id), &profile.name);
    }

    // Select the first profile if available
    if !profiles.is_empty() {
        profile_combo.set_active_id(Some(&profiles[0].id));
    }

    play_box.append(&profile_combo);

    // Add a play button
    let play_button = gtk::Button::with_label("Play");
    play_button.add_css_class("suggested-action");
    play_button.set_halign(gtk::Align::Center);
    play_button.set_hexpand(true);
    play_button.set_margin_top(20);
    play_button.set_margin_bottom(20);
    play_box.append(&play_button);

    // Connect the play button click event
    let plugin_id = plugin.get_id().to_string();
    let auth_session_clone = auth_session.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let profile_combo_clone = profile_combo.clone();
    play_button.connect_clicked(move |_| {
        // Get the selected profile ID
        if let Some(profile_id) = profile_combo_clone.active_id() {
            // Find the profile
            let profile_id_str = profile_id.to_string();

            // Check if the user is logged in
            let auth_session_guard = auth_session_clone.lock().unwrap();
            if let Some(auth_session) = &*auth_session_guard {
                // Show a toast notification
                let toast = adw::Toast::new(&format!("Launching Minecraft with profile: {}", profile_id_str));
                toast_overlay_clone.add_toast(toast);

                // TODO: Launch the game
            } else {
                // Show an error toast
                let toast = adw::Toast::new("You need to log in first");
                toast_overlay_clone.add_toast(toast);
            }
        } else {
            // Show an error toast
            let toast = adw::Toast::new("No profile selected");
            toast_overlay_clone.add_toast(toast);
        }
    });

    play_box
}

/// Build the profiles view for Minecraft
pub fn build_profiles_view(
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    plugin: &dyn GamePlugin,
) -> gtk::Box {
    // Create a vertical box for the profiles view
    let profiles_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    profiles_box.set_margin_top(20);
    profiles_box.set_margin_bottom(20);
    profiles_box.set_margin_start(20);
    profiles_box.set_margin_end(20);

    // Add a header
    let header = gtk::Label::new(Some("Minecraft Profiles"));
    header.add_css_class("title-2");
    header.set_halign(gtk::Align::Start);
    profiles_box.append(&header);

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(10);
    separator.set_margin_bottom(10);
    profiles_box.append(&separator);

    // Get profiles from the plugin
    let profiles = plugin.get_profiles();

    // Add a list box for profiles
    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.add_css_class("boxed-list");

    // Add profiles to the list box
    for profile in profiles {
        let row = adw::ActionRow::new();
        row.set_title(&profile.name);
        row.set_subtitle(&format!("Version: {}", profile.version));
        list_box.append(&row);
    }

    profiles_box.append(&list_box);

    // Add a button to add a new profile
    let add_button = gtk::Button::with_label("Add Profile");
    add_button.add_css_class("suggested-action");
    add_button.set_halign(gtk::Align::Center);
    add_button.set_margin_top(10);
    profiles_box.append(&add_button);

    profiles_box
}

/// Build the mods view for Minecraft
pub fn build_mods_view(
    window: &gtk::Window,
    toast_overlay: &adw::ToastOverlay,
    plugin: &dyn GamePlugin,
) -> gtk::Box {
    // Create a vertical box for the mods view
    let mods_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    mods_box.set_margin_top(20);
    mods_box.set_margin_bottom(20);
    mods_box.set_margin_start(20);
    mods_box.set_margin_end(20);

    // Add a header
    let header = gtk::Label::new(Some("Minecraft Mods"));
    header.add_css_class("title-2");
    header.set_halign(gtk::Align::Start);
    mods_box.append(&header);

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    separator.set_margin_top(10);
    separator.set_margin_bottom(10);
    mods_box.append(&separator);

    // Get mod search URL from the plugin
    let mod_search_url = plugin.get_mod_search_url();
    if let Some(url) = mod_search_url {
        // Add a label with the mod search URL
        let url_label = gtk::Label::new(Some(&format!("Mod Search URL: {}", url)));
        url_label.set_halign(gtk::Align::Start);
        url_label.set_margin_bottom(10);
        mods_box.append(&url_label);
    }

    // Add a search entry
    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Search for mods"));
    search_entry.set_margin_bottom(10);
    mods_box.append(&search_entry);

    // Add a scrolled window for the mods list
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_vexpand(true);
    scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    // Add a flow box for the mods
    let flow_box = gtk::FlowBox::new();
    flow_box.set_valign(gtk::Align::Start);
    flow_box.set_max_children_per_line(3);
    flow_box.set_selection_mode(gtk::SelectionMode::None);
    flow_box.set_homogeneous(true);
    flow_box.set_row_spacing(10);
    flow_box.set_column_spacing(10);
    scrolled_window.set_child(Some(&flow_box));
    mods_box.append(&scrolled_window);

    mods_box
}
