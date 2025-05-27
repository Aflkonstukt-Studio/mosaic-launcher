use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use log::{info, warn, error, debug};

use crate::games::minecraft::auth::{AuthManager, AuthSession};
use crate::config::{Config, ModLoader, GameType, save_config};
use crate::games::{GamePluginManager};
use crate::games::minecraft::{MinecraftPluginFactory};
use crate::file_manager::{FileManager};

use super::main_view::build_main_view;
use super::game_selector::build_game_selector;

pub struct MosaicApp {
    app: adw::Application,
    config: Rc<RefCell<Config>>,
    auth_manager: Rc<AuthManager>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
}

impl MosaicApp {
    pub fn new(config: &Config) -> Self {
        // Create the application
        info!("Creating application");
        let app = adw::Application::new(Some("xyz.aflkonstukt.launcher"), Default::default());

        // Create shared resources
        let config = Rc::new(RefCell::new(config.clone()));
        let auth_manager = Rc::new(AuthManager::new());

        // Create a single FileManager instance and wrap it in an Rc
        let file_manager = Rc::new(FileManager::new());

        let auth_session = Arc::new(Mutex::new(None));

        // Create the GamePluginManager
        let game_plugin_manager = Arc::new(Mutex::new(GamePluginManager::new(config.clone(), file_manager.clone())));

        // Register the MinecraftPluginFactory
        {
            let mut manager = game_plugin_manager.lock().unwrap();
            manager.register_factory(Box::new(MinecraftPluginFactory));
            manager.initialize_plugins();
        }

        info!("Application created");

        // Create the application
        MosaicApp {
            app,
            config,
            auth_manager,
            file_manager,
            auth_session,
            game_plugin_manager,
        }
    }

    pub fn run(&self) -> i32 {
        // Connect the activate signal
        let config = self.config.clone();
        let auth_manager = self.auth_manager.clone();
        let file_manager = self.file_manager.clone();
        let auth_session = self.auth_session.clone();
        let game_plugin_manager = self.game_plugin_manager.clone();

        info!("Connecting activate signal");
        self.app.connect_activate(move |app| {
            // Build the main window
            let window = build_main_window(
                app,
                config.clone(),
                auth_manager.clone(),
                file_manager.clone(),
                auth_session.clone(),
                game_plugin_manager.clone(),
            );

            // Show the window
            info!("Showing main window");
            window.present();
        });

        // Run the application
        self.app.run().into()
    }
}

/// Builds a game selector view with navigation logic
fn build_game_selector_view(
    window: &adw::ApplicationWindow,
    toast_overlay: &adw::ToastOverlay,
    stack: &gtk::Stack,
    config: Rc<RefCell<Config>>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
) -> gtk::Box {
    // Create a container for the game selector
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Build the game selector
    let game_selector = build_game_selector(
        window,
        toast_overlay,
        config.clone(),
    );

    // Create a wrapper for the list box to add custom behavior
    let list_box = game_selector.first_child()
        .and_then(|child| child.next_sibling())
        .and_then(|child| child.next_sibling())
        .and_then(|child| {
            if let Some(scrolled) = child.downcast_ref::<gtk::ScrolledWindow>() {
                scrolled.child()
            } else {
                None
            }
        })
        .and_then(|child| child.downcast::<gtk::ListBox>().ok());

    if let Some(list_box) = list_box {
        // Connect our custom row-selected handler
        let stack_clone = stack.clone();
        let config_clone = config.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let window_clone = window.clone();
        let auth_session_clone = auth_session.clone();
        let game_plugin_manager_clone = game_plugin_manager.clone();

        list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                if let Some(game_id) = unsafe { row.data::<String>("game-id") } {
                    // Convert NonNull<String> to String by dereferencing and cloning
                    let game_id_str = unsafe { game_id.as_ref().clone() };

                    // Update the selected game in the config
                    let mut config = config_clone.borrow_mut();
                    config.selected_game = Some(game_id_str.clone());

                    // Save the config
                    if let Err(e) = save_config(&config) {
                        error!("Failed to save config: {}", e);
                        let toast = adw::Toast::new(&format!("Failed to save game selection: {}", e));
                        toast_overlay_clone.add_toast(toast);
                    } else {
                        info!("Selected game: {}", game_id_str);

                        // Find the game to check its type
                        let game = config.games.iter().find(|g| g.id == game_id_str);

                        if let Some(game) = game {
                            // Navigate based on game type
                            match game.game_type {
                                GameType::Minecraft => {
                                    // For Minecraft, show the login screen
                                    info!("Navigating to login view for Minecraft");

                                    // Get the game plugin manager
                                    let game_plugin_manager = game_plugin_manager_clone.lock().unwrap();

                                    // Get the Minecraft plugin
                                    if let Some(plugin) = game_plugin_manager.get_plugin("minecraft") {
                                        // Call the plugin's handle_game_selection method
                                        if let Err(e) = plugin.handle_game_selection(
                                            window_clone.upcast_ref::<gtk::Window>(),
                                            &toast_overlay_clone,
                                            &stack_clone,
                                            auth_session_clone.clone(),
                                            config_clone.clone(),
                                        ) {
                                            error!("Failed to handle game selection: {}", e);
                                            let toast = adw::Toast::new(&format!("Failed to handle game selection: {}", e));
                                            toast_overlay_clone.add_toast(toast);
                                        }
                                    } else {
                                        error!("Minecraft plugin not found");
                                        let toast = adw::Toast::new("Minecraft plugin not found");
                                        toast_overlay_clone.add_toast(toast);
                                        stack_clone.set_visible_child_name("game_selector");
                                    }
                                },
                                _ => {
                                    // For other games, go directly to the main view
                                    info!("Navigating to main view for {}", game.name);
                                    stack_clone.set_visible_child_name("main");
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    container.append(&game_selector);
    container
}

fn build_main_window(
    app: &adw::Application,
    config: Rc<RefCell<Config>>,
    auth_manager: Rc<AuthManager>,
    file_manager: Rc<FileManager>,
    auth_session: Arc<Mutex<Option<AuthSession>>>,
    game_plugin_manager: Arc<Mutex<GamePluginManager>>,
) -> adw::ApplicationWindow {
    // Create the main window
    info!("Building main application window");
    let window = adw::ApplicationWindow::new(app);
    window.set_title(Some("Mosaic Launcher"));
    window.set_default_size(1200, 800);

    // Create a toast overlay for notifications
    let toast_overlay = adw::ToastOverlay::new();

    // Create a stack to switch between views
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.set_transition_duration(200);

    // Build a simple main view for now
    let main_view = gtk::Box::new(gtk::Orientation::Vertical, 10);
    main_view.set_margin_top(20);
    main_view.set_margin_bottom(20);
    main_view.set_margin_start(20);
    main_view.set_margin_end(20);

    // Add a header
    let header = gtk::Label::new(Some("Mosaic Launcher"));
    header.add_css_class("title-1");
    header.set_halign(gtk::Align::Center);
    main_view.append(&header);

    // Add a message
    let message = gtk::Label::new(Some("Select a game from the game selector to get started."));
    message.set_margin_top(20);
    message.set_halign(gtk::Align::Center);
    main_view.append(&message);
    stack.add_named(&main_view, Some("main"));
    info!("Main view built");

    // Build the game selector view
    let game_selector_view = build_game_selector_view(
        &window,
        &toast_overlay,
        &stack,
        config.clone(),
        game_plugin_manager.clone(),
        auth_session.clone(),
    );
    stack.add_named(&game_selector_view, Some("game_selector"));
    info!("Game selector view built");

    // Start with the game selector view
    stack.set_visible_child_name("game_selector");

    info!("Stack built");

    // Add the stack to the toast overlay
    toast_overlay.set_child(Some(&stack));

    // Add the toast overlay to the window
    window.set_content(Some(&toast_overlay));

    // Show the window
    info!("Main application window built");

    window
}
