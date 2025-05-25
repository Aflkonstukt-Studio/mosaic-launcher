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
use crate::minecraft::{MinecraftManager, VersionManifest};

pub fn build_play_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
) -> gtk::Box {
    info!("Building play view");
    // Create a vertical box for the play view
    let play_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    play_box.set_hexpand(true);
    play_box.set_vexpand(true);
    play_box.set_size_request(800, 600);

    // Create a header bar
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&gtk::Label::new(Some("Play"))));
    header.set_show_start_title_buttons(false);
    play_box.append(&header);

    // Create a content box
    let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
    content.set_margin_top(20);
    content.set_margin_bottom(20);
    content.set_margin_start(20);
    content.set_margin_end(20);
    play_box.append(&content);

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

    // Add a spacer
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    content.append(&spacer);

    // Add a play button
    let play_button = gtk::Button::with_label("Play");
    play_button.add_css_class("suggested-action");
    play_button.add_css_class("pill");
    play_button.set_halign(gtk::Align::Center);
    play_button.set_size_request(200, 50);
    content.append(&play_button);

    // Add a login required message (hidden by default)
    let login_message = gtk::Label::new(Some("You need to sign in to play Minecraft"));
    login_message.add_css_class("caption");
    login_message.add_css_class("dim-label");
    login_message.set_margin_top(10);
    login_message.set_halign(gtk::Align::Center);
    login_message.set_visible(false);
    content.append(&login_message);

    // Check if user is logged in and update UI accordingly
    if auth_session.lock().unwrap().is_none() {
        play_button.set_sensitive(false);
        login_message.set_visible(true);
    }

    // Connect the play button
    let config_clone = config.clone();
    let minecraft_manager_clone = minecraft_manager.clone();
    let auth_session_clone = auth_session.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let profile_combo_clone = profile_combo.clone();
    let login_message_clone = login_message.clone();

    play_button.connect_clicked(move |button| {
        // Check if user is logged in
        if auth_session_clone.lock().unwrap().is_none() {
            let toast = adw::Toast::new("You need to sign in to play Minecraft");
            toast_overlay_clone.add_toast(toast);
            login_message_clone.set_visible(true);
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
        let profile_clone = profile.clone();

        // Update last used profile
        config_clone.borrow_mut().last_used_profile = Some(profile.id.clone());
        let _ = save_config(&config_clone.borrow());

        // Show a loading indicator
        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_child(Some(&spinner));
        button.set_sensitive(false);

        // Clone references for the async closure
        let minecraft_manager = minecraft_manager_clone.clone();
        let auth_session = auth_session_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let button = button.clone();

        // Launch the game
        gtk::glib::spawn_future_local(async move {
            let auth = auth_session.lock().unwrap();
            // We know auth_session is Some because we checked above
            let auth = auth.as_ref().unwrap();

            match minecraft_manager.lock().unwrap().launch_game(&profile_clone, auth) {
                Ok(_) => {
                    let toast = adw::Toast::new(&format!("Launched Minecraft with profile '{}'", profile_clone.name));
                    toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    let toast = adw::Toast::new(&format!("Failed to launch Minecraft: {}", e));
                    toast_overlay.add_toast(toast);
                }
            }

            // Reset the button
            button.set_label("Play");
            button.set_sensitive(true);
        });
    });

    play_box
}
