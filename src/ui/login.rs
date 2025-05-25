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

use crate::auth::{AuthManager, AuthSession};
use crate::config::Config;

pub fn build_login_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    auth_manager: Rc<AuthManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    stack: &gtk::Stack,
    config: Rc<RefCell<Config>>,
) -> gtk::Box {
    // Create a vertical box for the login view
    let login_box = gtk::Box::new(gtk::Orientation::Vertical, 20);
    login_box.set_margin_top(50);
    login_box.set_margin_bottom(50);
    login_box.set_margin_start(50);
    login_box.set_margin_end(50);
    login_box.set_halign(gtk::Align::Center);
    login_box.set_valign(gtk::Align::Center);

    // Add a logo or title
    let title = gtk::Label::new(Some("Mosaic Launcher"));
    title.add_css_class("title-1");
    login_box.append(&title);

    // Add a subtitle
    let subtitle = gtk::Label::new(Some("Sign in with your Microsoft account to play Minecraft"));
    subtitle.add_css_class("title-4");
    login_box.append(&subtitle);

    // Add some space
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    login_box.append(&spacer);

    // Add a login button
    let login_button = gtk::Button::with_label("Sign in with Microsoft");
    login_button.add_css_class("suggested-action");
    login_button.add_css_class("pill");
    login_button.set_halign(gtk::Align::Center);

    // Clone references for the closure
    let auth_manager_clone = auth_manager.clone();
    let auth_session_clone = auth_session.clone();
    let stack_clone = stack.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();
    let config_clone = config.clone();

    // Connect the login button click event
    let login_button_clone = login_button.clone();
    let login_button_for_async = login_button.clone();
    login_button.connect_clicked(move |_| {
        // Show a loading indicator
        let spinner = gtk::Spinner::new();
        spinner.start();
        login_button_clone.set_child(Some(&spinner));
        login_button_clone.set_sensitive(false);

        // Clone references for the async closure
        let auth_manager = auth_manager_clone.clone();
        let auth_session = auth_session_clone.clone();
        let stack = stack_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let window = window_clone.clone();
        let login_button = login_button_for_async.clone();
        let config = config_clone.clone();

        // Start the authentication process
        gtk::glib::spawn_future_local(async move {
            // Step 1: Get the device code
            match auth_manager.start_login() {
                Ok(device_auth) => {
                    // Create a custom dialog to display the device code
                    let dialog = gtk::Dialog::new();
                    dialog.set_modal(true);
                    dialog.set_transient_for(Some(&window));
                    dialog.set_default_width(400);

                    // Create a header bar with title
                    let header_bar = gtk::HeaderBar::new();
                    header_bar.set_title_widget(Some(&gtk::Label::builder()
                        .label("Sign in with Microsoft")
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

                    // Add instructions
                    let instructions_label = gtk::Label::new(Some("To sign in, use a web browser to open the page:"));
                    instructions_label.set_halign(gtk::Align::Start);
                    content_area.append(&instructions_label);

                    // Add URL box with copy button
                    let url_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let url = device_auth.verification_uri().to_string();
                    let url_label = gtk::Label::new(Some(&url));
                    url_label.set_selectable(true);
                    url_label.set_hexpand(true);
                    url_label.set_halign(gtk::Align::Start);
                    url_box.append(&url_label);

                    // Add open link button
                    let open_link_button = gtk::Button::from_icon_name("web-browser-symbolic");
                    open_link_button.set_tooltip_text(Some("Open in browser"));
                    open_link_button.set_valign(gtk::Align::Center);
                    url_box.append(&open_link_button);

                    // Connect open link button
                    let url_clone = url.clone();
                    let config_clone = config.clone();
                    let toast_overlay_clone = toast_overlay.clone();
                    open_link_button.connect_clicked(move |_| {
                        // Try to launch the browser using the standard method first
                        let result = match gtk::gio::AppInfo::launch_default_for_uri(&url_clone, None::<&gtk::gio::AppLaunchContext>) {
                            Ok(_) => Ok(()),
                            Err(e) => {
                                // If it fails and the error might be sandbox-related, try xdg-open as fallback on Linux
                                if cfg!(target_os = "linux") && (e.to_string().contains("CanCreateUserNamespace") || e.to_string().contains("EPERM")) {
                                    warn!("Failed to open browser with GTK due to sandbox error: {}. Trying xdg-open directly.", e);
                                    match std::process::Command::new("xdg-open")
                                        .arg(&url_clone)
                                        .spawn() {
                                        Ok(_) => Ok(()),
                                        Err(e2) => Err(format!("Failed to open URL: {} (and fallback failed: {})", e, e2)),
                                    }
                                } else {
                                    Err(format!("Failed to open URL: {}", e))
                                }
                            }
                        };

                        // Handle any errors
                        if let Err(e) = result {
                            error!("Failed to open URL: {}", e);
                            let toast = adw::Toast::new(&format!("Failed to open browser: {}. Please copy the URL and open it manually.", e));
                            toast_overlay_clone.add_toast(toast);
                        }
                    });

                    content_area.append(&url_box);

                    // Add code instructions
                    let code_instructions_label = gtk::Label::new(Some("And enter the code:"));
                    code_instructions_label.set_halign(gtk::Align::Start);
                    content_area.append(&code_instructions_label);

                    // Add code box with copy button
                    let code_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let code = device_auth.user_code().secret().to_string();
                    let code_label = gtk::Label::builder()
                        .label(&code)
                        .css_classes(vec!["title-2"])
                        .selectable(true)
                        .build();
                    code_label.set_hexpand(true);
                    code_label.set_halign(gtk::Align::Start);
                    code_box.append(&code_label);

                    // Add copy button
                    let copy_button = gtk::Button::from_icon_name("edit-copy-symbolic");
                    copy_button.set_tooltip_text(Some("Copy to clipboard"));
                    copy_button.set_valign(gtk::Align::Center);
                    code_box.append(&copy_button);

                    // Connect copy button
                    let code_clone = code.clone();
                    let window_clone = window.clone();
                    let toast_overlay_for_copy = toast_overlay.clone();
                    copy_button.connect_clicked(move |_| {
                        let clipboard = window_clone.clipboard();
                        clipboard.set_text(&code_clone);

                        // Show a toast notification
                        let toast = adw::Toast::new("Code copied to clipboard");
                        toast.set_timeout(2);
                        toast_overlay_for_copy.add_toast(toast);
                    });

                    content_area.append(&code_box);

                    // Add a separator
                    let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
                    separator.set_margin_top(8);
                    separator.set_margin_bottom(8);
                    content_area.append(&separator);

                    // Add status label
                    let status_label = gtk::Label::new(Some("Waiting for you to sign in..."));
                    status_label.set_margin_top(8);
                    content_area.append(&status_label);

                    // Add action buttons
                    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
                    dialog.add_button("I've signed in", gtk::ResponseType::Ok);
                    dialog.set_default_response(gtk::ResponseType::Ok);

                    // Set up auto-checking for authentication
                    let auth_manager_for_auto = auth_manager.clone();
                    let device_auth_for_auto = device_auth.clone();
                    let status_label_clone = status_label.clone();
                    let dialog_clone = dialog.clone();

                    // Create a cancellation flag
                    let cancelled = Rc::new(RefCell::new(false));
                    let cancelled_clone = cancelled.clone();

                    // Start the auto-check process
                    let check_id = gtk::glib::timeout_add_local(
                        std::time::Duration::from_secs(5),
                        move || {
                            // Check if the dialog has been cancelled
                            if *cancelled_clone.borrow() {
                                return gtk::glib::ControlFlow::Break;
                            }

                            // Clone for the async closure
                            let auth_manager = auth_manager_for_auto.clone();
                            let device_auth = device_auth_for_auto.clone();
                            let status_label = status_label_clone.clone();
                            let dialog = dialog_clone.clone();
                            let cancelled = cancelled_clone.clone();

                            // Update status label
                            status_label.set_text("Checking authentication status...");

                            // Check if the user has completed authentication
                            gtk::glib::spawn_future_local(async move {
                                // Try to complete the login without waiting for user input
                                match auth_manager.complete_login(device_auth).await {
                                    Ok(_) => {
                                        // Authentication successful, close the dialog with OK response
                                        if !*cancelled.borrow() {
                                            status_label.set_text("Authentication successful! Proceeding...");
                                            dialog.response(gtk::ResponseType::Ok);
                                        }
                                    }
                                    Err(_) => {
                                        // Authentication not yet complete, continue waiting
                                        if !*cancelled.borrow() {
                                            status_label.set_text("Waiting for you to sign in...");
                                        }
                                    }
                                }
                            });

                            // Continue the timeout
                            gtk::glib::ControlFlow::Continue
                        },
                    );

                    // Set up the response handler
                    let auth_manager_clone = auth_manager.clone();
                    let auth_session_clone = auth_session.clone();
                    let stack_clone = stack.clone();
                    let toast_overlay_clone = toast_overlay.clone();
                    let login_button_clone = login_button.clone();
                    let device_auth_clone = device_auth.clone();
                    let cancelled_for_response = cancelled.clone();

                    dialog.connect_response(move |dialog, response| {
                        // Set the cancelled flag to true to stop the auto-check
                        *cancelled_for_response.borrow_mut() = true;

                        dialog.destroy();

                        if response == gtk::ResponseType::Ok {
                            // Clone references for the async closure
                            let auth_manager = auth_manager_clone.clone();
                            let auth_session = auth_session_clone.clone();
                            let stack = stack_clone.clone();
                            let toast_overlay = toast_overlay_clone.clone();
                            let login_button = login_button_clone.clone();
                            let device_auth = device_auth_clone.clone();

                            // Show a loading indicator
                            let loading_toast = adw::Toast::new("Completing sign-in process...");
                            toast_overlay.add_toast(loading_toast);

                            // Step 2: Poll for token and complete login
                            gtk::glib::spawn_future_local(async move {
                                match auth_manager.complete_login(device_auth).await {
                                    Ok(session) => {
                                        // Store the auth session
                                        *auth_session.lock().unwrap() = Some(session);

                                        // Show a success toast
                                        let toast = adw::Toast::new("Successfully signed in");
                                        toast_overlay.add_toast(toast);

                                        // Switch to the main view
                                        stack.set_visible_child_name("main");
                                    }
                                    Err(e) => {
                                        // Check if the error is related to sandbox issues
                                        let error_msg = e.to_string();
                                        if error_msg.contains("CanCreateUserNamespace") || error_msg.contains("EPERM") {
                                            // This is likely a sandbox error
                                            let toast = adw::Toast::new(
                                                "Login failed due to sandbox permissions. As a last resort, you can disable sandbox mode in Settings."
                                            );
                                            toast.set_timeout(0); // Don't auto-hide
                                            toast_overlay.add_toast(toast);

                                            // Show a second toast with instructions
                                            let instructions_toast = adw::Toast::new(
                                                "To disable sandbox: Settings > Advanced > Disable Sandbox"
                                            );
                                            instructions_toast.set_timeout(0); // Don't auto-hide
                                            toast_overlay.add_toast(instructions_toast);
                                        } else {
                                            // This is a general error
                                            let toast = adw::Toast::new(&format!("Login failed: {}", e));
                                            toast_overlay.add_toast(toast);
                                        }

                                        // Reset the login button
                                        login_button.set_label("Sign in with Microsoft");
                                        login_button.set_sensitive(true);
                                    }
                                }
                            });
                        } else {
                            // User cancelled, reset the login button
                            login_button.set_label("Sign in with Microsoft");
                            login_button.set_sensitive(true);
                        }
                    });

                    // Show the dialog
                    dialog.present();
                }
                Err(e) => {
                    // Failed to start login process
                    let toast = adw::Toast::new(&format!("Failed to start login process: {}", e));
                    toast_overlay.add_toast(toast);

                    // Reset the login button
                    login_button.set_label("Sign in with Microsoft");
                    login_button.set_sensitive(true);
                }
            }
        });
    });
    login_box.append(&login_button);

    // Add an "Offline Mode" button
    let offline_button = gtk::Button::with_label("Play in Offline Mode");
    offline_button.add_css_class("pill");
    offline_button.set_halign(gtk::Align::Center);
    offline_button.set_margin_top(10);

    // Clone references for the closure
    let auth_manager_clone = auth_manager.clone();
    let auth_session_clone = auth_session.clone();
    let stack_clone = stack.clone();
    let toast_overlay_clone = toast_overlay.clone();
    let window_clone = window.clone();

    // Connect the offline button click event
    offline_button.connect_clicked(move |_| {
        // Create a dialog for entering a username
        let dialog = gtk::Dialog::new();
        dialog.set_title(Some("Offline Mode"));
        dialog.set_modal(true);
        dialog.set_transient_for(Some(&window_clone));
        dialog.set_default_width(350);

        // Create a header bar with title
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_title_widget(Some(&gtk::Label::builder()
            .label("Enter Username")
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

        // Add instructions
        let instructions_label = gtk::Label::new(Some("Enter a username to use in offline mode:"));
        instructions_label.set_halign(gtk::Align::Start);
        content_area.append(&instructions_label);

        // Add username entry
        let username_entry = gtk::Entry::new();
        username_entry.set_placeholder_text(Some("Username"));
        username_entry.set_activates_default(true);
        content_area.append(&username_entry);

        // Add warning label
        let warning_label = gtk::Label::new(Some("Warning: Offline mode only works for singleplayer or cracked servers."));
        warning_label.add_css_class("caption");
        warning_label.set_halign(gtk::Align::Start);
        content_area.append(&warning_label);

        // Add action buttons
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);
        let ok_button = dialog.add_button("Play Offline", gtk::ResponseType::Ok);
        dialog.set_default_response(gtk::ResponseType::Ok);

        // Clone references for the response handler
        let auth_manager = auth_manager_clone.clone();
        let auth_session = auth_session_clone.clone();
        let stack = stack_clone.clone();
        let toast_overlay = toast_overlay_clone.clone();
        let username_entry_clone = username_entry.clone();

        // Connect the response signal
        dialog.connect_response(move |dialog, response| {
            dialog.destroy();

            if response == gtk::ResponseType::Ok {
                let username = username_entry_clone.text().to_string();

                // Validate username
                if username.is_empty() {
                    let toast = adw::Toast::new("Username cannot be empty");
                    toast_overlay.add_toast(toast);
                    return;
                }

                // Create an offline session
                match auth_manager.create_offline_session(&username) {
                    Ok(session) => {
                        // Store the auth session
                        *auth_session.lock().unwrap() = Some(session);

                        // Show a success toast
                        let toast = adw::Toast::new(&format!("Playing as {} (offline mode)", username));
                        toast_overlay.add_toast(toast);

                        // Switch to the main view
                        stack.set_visible_child_name("main");
                    }
                    Err(e) => {
                        // Show error toast
                        let toast = adw::Toast::new(&format!("Failed to create offline session: {}", e));
                        toast_overlay.add_toast(toast);
                    }
                }
            }
        });

        // Show the dialog
        dialog.present();
    });
    login_box.append(&offline_button);

    // Add a "Continue without logging in" button
    let continue_button = gtk::Button::with_label("Continue without logging in");
    continue_button.add_css_class("pill");
    continue_button.set_halign(gtk::Align::Center);
    continue_button.set_margin_top(10);

    // Connect the continue button click event
    let stack_clone = stack.clone();
    let toast_overlay_clone = toast_overlay.clone();
    continue_button.connect_clicked(move |_| {
        // Show a toast notification
        let toast = adw::Toast::new("You can browse and download mods, but you'll need to sign in to play Minecraft");
        toast_overlay_clone.add_toast(toast);
        let stack_clone = stack_clone.clone();

        glib::idle_add_local_once(move || {
            info!("Switching to main view");
            stack_clone.set_visible_child_name("main");
            info!("Switch completed");
        });
    });
    login_box.append(&continue_button);

    // Add some space
    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    login_box.append(&spacer);

    login_box
}
