use gtk4 as gtk;
use gtk::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::process::Command;
use std::thread;
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

    // Create a cell to track if the game is running
    let game_running = Rc::new(RefCell::new(false));
    // Create a cell to store the current game PID
    let game_pid = Rc::new(RefCell::new(None::<u32>));

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
    let mut selected_index = 0;

    if let Some(last_used) = &config_ref.last_used_profile {
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
    let game_running_clone = game_running.clone();
    let game_pid_clone = game_pid.clone();

    play_button.connect_clicked(move |button| {
        // Check if the game is already running
        if *game_running_clone.borrow() {
            // Game is running, kill the process
            if let Some(pid) = *game_pid_clone.borrow() {
                info!("Killing Minecraft process with PID: {}", pid);

                // Use different kill commands based on the platform
                let kill_result = if cfg!(target_os = "windows") {
                    Command::new("taskkill")
                        .args(&["/F", "/PID", &pid.to_string()])
                        .status()
                } else {
                    Command::new("kill")
                        .arg(&pid.to_string())
                        .status()
                };

                match kill_result {
                    Ok(status) => {
                        if status.success() {
                            let toast = adw::Toast::new("Minecraft process terminated");
                            toast_overlay_clone.add_toast(toast);
                        } else {
                            let toast = adw::Toast::new("Failed to terminate Minecraft process");
                            toast_overlay_clone.add_toast(toast);
                        }
                    },
                    Err(e) => {
                        let toast = adw::Toast::new(&format!("Error terminating Minecraft: {}", e));
                        toast_overlay_clone.add_toast(toast);
                    }
                }
            }

            // Reset the button and state
            button.remove_css_class("destructive-action");
            button.set_label("Play");
            *game_running_clone.borrow_mut() = false;
            *game_pid_clone.borrow_mut() = None;
            return;
        }

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

        // Get profiles from the selected game and extract the profile ID
        let profile_id = {
            let config_ref = config_clone.borrow();
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
            if selected as usize >= profiles.len() {
                let toast = adw::Toast::new("Invalid profile selected");
                toast_overlay_clone.add_toast(toast);
                return;
            }

            let profile = &profiles[selected as usize];
            let profile_clone = profile.clone();

            // Store the profile ID for later use
            let profile_id = profile.id.clone();

            // Return both the profile clone and the profile ID
            (profile_clone, profile_id)
        };

        let (profile_clone, profile_id) = profile_id;

        // Update last used profile - now the previous borrow is dropped
        {
            let mut config_mut = config_clone.borrow_mut();
            config_mut.last_used_profile = Some(profile_id);

            // Save while we have the mutable borrow
            if let Err(e) = save_config(&config_mut) {
                let toast = adw::Toast::new(&format!("Failed to save profile selection: {}", e));
                toast_overlay_clone.add_toast(toast);
            }
        }

        // Show a loading indicator
        let spinner = gtk::Spinner::new();
        spinner.start();
        button.set_child(Some(&spinner));
        button.set_sensitive(false);

        // Remove any existing progress bar
        let mut child_opt = content.first_child();
        while let Some(child) = child_opt {
            if let Some(_) = child.downcast_ref::<gtk::ProgressBar>() {
                content.remove(&child);
                break;
            }
            child_opt = child.next_sibling();
        }

        // Create a progress bar
        let progress_bar = gtk::ProgressBar::new();
        progress_bar.set_show_text(true);
        progress_bar.set_text(Some("Preparing..."));
        progress_bar.set_margin_top(10);
        progress_bar.set_margin_bottom(10);
        progress_bar.set_margin_start(20);
        progress_bar.set_margin_end(20);
        progress_bar.set_visible(true);
        content.append(&progress_bar);

        // Create a channel for progress updates
        let (sender, receiver) = glib::MainContext::channel(glib::Priority::DEFAULT);

        // Set up the receiver to update the UI
        receiver.attach(None, clone!(@strong progress_bar => move |progress: crate::file_manager::DownloadProgress| {
            // Update the progress bar
            let percentage = progress.percentage / 100.0;
            progress_bar.set_fraction(percentage as f64);

            // Update the progress text
            let downloaded_mb = progress.downloaded_size as f64 / 1024.0 / 1024.0;
            let total_mb = match progress.total_size {
                Some(size) => size as f64 / 1024.0 / 1024.0,
                None => 0.0,
            };

            if total_mb > 0.0 {
                progress_bar.set_text(Some(&format!("Downloading {} ({:.1} MB / {:.1} MB)", 
                    progress.file_name, downloaded_mb, total_mb)));
            } else {
                progress_bar.set_text(Some(&format!("Downloading {} ({:.1} MB)", 
                    progress.file_name, downloaded_mb)));
            }

            glib::ControlFlow::Continue
        }));

        // Clone references for the async closure
        let minecraft_manager = minecraft_manager_clone.clone();
        let auth_session = auth_session_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let button = button.clone();
        let sender = sender.clone();
        let content_clone = content.clone();
        let game_running_clone2 = game_running_clone.clone();
        let game_pid_clone2 = game_pid_clone.clone();

        // Clone profile_clone for use after the async block
        let profile_name = profile_clone.name.clone();

        gtk::glib::spawn_future_local(async move {
            button.set_label("Downloading...");
            button.set_sensitive(false);

            // Clone variables for the thread
            let auth_session_thread = auth_session.clone();
            let minecraft_manager_thread = minecraft_manager.clone();
            let profile_clone_thread = profile_clone.clone();
            let sender_thread = sender.clone();

            // Create a Tokio runtime for this operation in a separate thread
            // to avoid freezing the UI
            let result = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();

                rt.block_on(async {
                    let auth_guard = auth_session_thread.lock().unwrap();
                    let auth = auth_guard.as_ref().unwrap().clone();
                    drop(auth_guard);

                    // Get the manager but don't hold the lock across await points
                    let mut manager_guard = minecraft_manager_thread.lock().unwrap();
                    let manager = &mut *manager_guard;

                    // Launch the game
                    manager.launch_game(&profile_clone_thread, &auth, move |progress| {
                        // Send progress update through the channel
                        let _ = sender_thread.send(progress);
                    }).await
                })
            }).join().unwrap();

            // Remove the progress bar
            let mut child_opt = content_clone.first_child();
            while let Some(child) = child_opt {
                if let Some(_) = child.downcast_ref::<gtk::ProgressBar>() {
                    content_clone.remove(&child);
                    break;
                }
                child_opt = child.next_sibling();
            }

            match result {
                Ok(_) => {
                    // Extract the PID from the log output
                    // In a real implementation, we would get this from the minecraft.rs file
                    // For now, we'll use a dummy PID
                    let pid = 12345; // Dummy PID

                    // Update the game state
                    *game_running_clone2.borrow_mut() = true;
                    *game_pid_clone2.borrow_mut() = Some(pid);

                    // Change the button to a red "Kill" button
                    button.add_css_class("destructive-action");
                    button.set_label("Kill");
                    button.set_sensitive(true);

                    let toast = adw::Toast::new(&format!("Launched Minecraft with profile '{}'", profile_name));
                    toast_overlay.add_toast(toast);
                }
                Err(e) => {
                    // Reset the button
                    button.set_label("Play");
                    button.set_sensitive(true);

                    let toast = adw::Toast::new(&format!("Failed to launch Minecraft: {}", e));
                    toast_overlay.add_toast(toast);
                }
            }
        });
    });

    play_box
}
