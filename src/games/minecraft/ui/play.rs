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

use crate::config::{Config, save_config};
use crate::games::minecraft::{MinecraftManager, VersionManifest};
use crate::games::minecraft::auth::AuthSession;

pub fn build_play_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    config: Rc<RefCell<Config>>,
    minecraft_manager: Arc<Mutex<MinecraftManager>>,
    version_manifest: Arc<Mutex<Option<VersionManifest>>>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
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

    // Add a profile selector section
    let profile_section = gtk::Box::new(gtk::Orientation::Vertical, 10);
    profile_section.set_margin_top(10);
    profile_section.set_margin_bottom(10);

    // Add a header for the profile section
    let profile_header = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let profile_label = gtk::Label::new(Some("Profiles"));
    profile_label.add_css_class("title-3");
    profile_label.set_halign(gtk::Align::Start);
    profile_label.set_hexpand(true);
    profile_header.append(&profile_label);

    // Add a refresh button
    let refresh_button = gtk::Button::new();
    refresh_button.set_icon_name("view-refresh-symbolic");
    refresh_button.set_tooltip_text(Some("Refresh profiles"));
    refresh_button.add_css_class("flat");
    profile_header.append(&refresh_button);

    profile_section.append(&profile_header);

    // Add a separator
    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
    profile_section.append(&separator);

    // Create a scrolled window for the profiles
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_min_content_height(200);
    scrolled_window.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    // Create a list box for the profiles
    let profile_list = gtk::ListBox::new();
    profile_list.add_css_class("boxed-list");
    scrolled_window.set_child(Some(&profile_list));
    profile_section.append(&scrolled_window);

    // Add the profile section to the content
    content.append(&profile_section);

    // Get the selected game and its profiles
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
    let last_used_profile_id = config_ref.last_used_profile.clone();

    // Populate the profile list
    for profile in profiles {
        // Create a row for the profile
        let row = adw::ActionRow::new();
        row.set_title(&profile.name);

        // Add subtitle with version and modloader info
        let mut subtitle = format!("Minecraft {}", profile.version);
        if let Some(mod_loader) = &profile.mod_loader {
            use crate::config::ModLoader;
            match mod_loader {
                ModLoader::Forge => subtitle.push_str(" with Forge"),
                ModLoader::Fabric => subtitle.push_str(" with Fabric"),
                ModLoader::Quilt => subtitle.push_str(" with Quilt"),
                ModLoader::NeoForge => subtitle.push_str(" with NeoForge"),
                ModLoader::None => {}
            }

            if let Some(version) = &profile.mod_loader_version {
                subtitle.push_str(&format!(" {}", version));
            }
        }
        row.set_subtitle(&subtitle);

        // Add memory info as a suffix
        if let Some(memory) = profile.memory {
            let memory_label = gtk::Label::new(Some(&format!("{}MB", memory)));
            memory_label.add_css_class("dim-label");
            memory_label.add_css_class("numeric");
            row.add_suffix(&memory_label);
        }

        // Store the profile ID in the row
        unsafe { row.set_data("profile-id", profile.id.clone()); }

        // Add the row to the list box
        profile_list.append(&row);

        // Select the row if it's the last used profile
        if let Some(last_used) = &last_used_profile_id {
            if last_used == &profile.id {
                profile_list.select_row(Some(&row));
            }
        }
    }

    // If no row is selected, select the first one
    if profile_list.selected_row().is_none() && !profiles.is_empty() {
        if let Some(row) = profile_list.row_at_index(0) {
            profile_list.select_row(Some(&row));
        }
    }

    // Connect the row-selected signal
    let config_clone = config.clone();
    let toast_overlay_clone = toast_overlay.clone();
    profile_list.connect_row_selected(move |_, row| {
        if let Some(row) = row {
            if let Some(profile_id) = unsafe { row.data::<String>("profile-id") } {
                // Convert NonNull<String> to String by dereferencing and cloning
                let profile_id_str = unsafe { profile_id.as_ref().clone() };

                // Update the last used profile in the config
                let mut config = config_clone.borrow_mut();
                config.last_used_profile = Some(profile_id_str.clone());

                // Save the config
                if let Err(e) = save_config(&config) {
                    error!("Failed to save config: {}", e);
                    let toast = adw::Toast::new(&format!("Failed to save profile selection: {}", e));
                    toast_overlay_clone.add_toast(toast);
                }
            }
        }
    });

    // Connect the refresh button
    let profile_list_clone = profile_list.clone();
    let config_clone = config.clone();
    refresh_button.connect_clicked(move |_| {
        // Clear the list box
        while let Some(child) = profile_list_clone.first_child() {
            profile_list_clone.remove(&child);
        }

        // Get the selected game and its profiles
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
        let last_used_profile_id = config_ref.last_used_profile.clone();

        // Populate the profile list
        for profile in profiles {
            // Create a row for the profile
            let row = adw::ActionRow::new();
            row.set_title(&profile.name);

            // Add subtitle with version and modloader info
            let mut subtitle = format!("Minecraft {}", profile.version);
            if let Some(mod_loader) = &profile.mod_loader {
                use crate::config::ModLoader;
                match mod_loader {
                    ModLoader::Forge => subtitle.push_str(" with Forge"),
                    ModLoader::Fabric => subtitle.push_str(" with Fabric"),
                    ModLoader::Quilt => subtitle.push_str(" with Quilt"),
                    ModLoader::NeoForge => subtitle.push_str(" with NeoForge"),
                    ModLoader::None => {}
                }

                if let Some(version) = &profile.mod_loader_version {
                    subtitle.push_str(&format!(" {}", version));
                }
            }
            row.set_subtitle(&subtitle);

            // Add memory info as a suffix
            if let Some(memory) = profile.memory {
                let memory_label = gtk::Label::new(Some(&format!("{}MB", memory)));
                memory_label.add_css_class("dim-label");
                memory_label.add_css_class("numeric");
                row.add_suffix(&memory_label);
            }

            // Store the profile ID in the row
            unsafe { row.set_data("profile-id", profile.id.clone()); }

            // Add the row to the list box
            profile_list_clone.append(&row);

            // Select the row if it's the last used profile
            if let Some(last_used) = &last_used_profile_id {
                if last_used == &profile.id {
                    profile_list_clone.select_row(Some(&row));
                }
            }
        }

        // If no row is selected, select the first one
        if profile_list_clone.selected_row().is_none() && !profiles.is_empty() {
            if let Some(row) = profile_list_clone.row_at_index(0) {
                profile_list_clone.select_row(Some(&row));
            }
        }
    });

    // Add a smaller spacer to position the play button higher
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    spacer.set_size_request(-1, 50); // Limit the height to 50 pixels
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

    // Connect the play button
    let config_clone = config.clone();
    let minecraft_manager_clone = minecraft_manager.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let profile_list_clone = profile_list.clone();
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

        // Get the selected profile from the profile list
        let selected_row = profile_list_clone.selected_row();
        if selected_row.is_none() {
            let toast = adw::Toast::new("No profile selected");
            toast_overlay_clone.add_toast(toast);
            return;
        }

        // Get the profile ID from the selected row
        let profile_id = {
            let row = selected_row.unwrap();
            let profile_id = unsafe { row.data::<String>("profile-id") };
            if profile_id.is_none() {
                let toast = adw::Toast::new("Invalid profile selected");
                toast_overlay_clone.add_toast(toast);
                return;
            }

            // Convert NonNull<String> to String by dereferencing and cloning
            let profile_id_str = unsafe { profile_id.unwrap().as_ref().clone() };

            // Get the profile from the config
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

            let profile = game.profiles.iter()
                .find(|p| p.id == profile_id_str)
                .cloned();

            if profile.is_none() {
                let toast = adw::Toast::new("Profile not found");
                toast_overlay_clone.add_toast(toast);
                return;
            }

            let profile_clone = profile.unwrap();

            // Return both the profile clone and the profile ID
            (profile_clone, profile_id_str)
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
        let toast_overlay = toast_overlay_clone.clone();
        let button = button.clone();
        let sender = sender.clone();
        let content_clone = content.clone();
        let game_running_clone2 = game_running_clone.clone();
        let game_pid_clone2 = game_pid_clone.clone();
        let auth_session_clone = auth_session.clone();

        // Clone profile_clone for use after the async block
        let profile_name = profile_clone.name.clone();

        gtk::glib::spawn_future_local(async move {
            button.set_label("Downloading...");
            button.set_sensitive(false);

            // Clone variables for the thread
            let minecraft_manager_thread = minecraft_manager.clone();
            let profile_clone_thread = profile_clone.clone();
            let sender_thread = sender.clone();
            let auth_session_thread = auth_session_clone.clone();

            // Create a Tokio runtime for this operation in a separate thread
            // to avoid freezing the UI
            let result = std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();

                // Get the auth session
                let auth = {
                    let auth_guard = auth_session_thread.lock().unwrap();
                    auth_guard.as_ref().unwrap().clone()
                };

                // Get the manager
                let mut manager_guard = minecraft_manager_thread.lock().unwrap();

                // Launch the game
                rt.block_on(manager_guard.launch_game(&profile_clone_thread, &auth, move |progress| {
                    // Send progress update through the channel
                    let _ = sender_thread.send(progress);
                }))
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
                Ok(pid) => {
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
