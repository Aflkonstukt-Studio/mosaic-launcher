use anyhow::Result;
use log::{info, error};
use mosaic_launcher::config;
use mosaic_launcher::ui::MosaicApp;

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();
    info!("Starting Mosaic Launcher");

    // Load configuration
    let config = config::load_config().unwrap_or_else(|e| {
        error!("Failed to load configuration: {}", e);
        config::default_config()
    });

    // Initialize the UI
    let app = MosaicApp::new(&config);

    // Run the application (this starts the GTK main loop)
    let exit_code = app.run();

    info!("Exiting Mosaic Launcher with code {}", exit_code);
    Ok(())
}
