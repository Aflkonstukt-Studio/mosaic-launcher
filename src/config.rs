use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use log::{info, warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub executable: Option<PathBuf>,
    pub game_directory: PathBuf,
    pub profiles: Vec<Profile>,
    pub game_type: GameType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameType {
    Minecraft,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub games: Vec<Game>,
    pub selected_game: Option<String>,
    pub last_used_profile: Option<String>,
    pub theme: Theme,
    pub max_memory: u32, // in MB
    pub java_arguments: Vec<String>,
    pub java_path: Option<PathBuf>,
    pub disable_sandbox: bool, // Disable sandbox mode for games
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub version: String,
    pub mod_loader: Option<ModLoader>,
    pub mod_loader_version: Option<String>,
    pub mods: Vec<Mod>,
    pub game_directory: Option<PathBuf>,
    pub resolution: Option<(u32, u32)>,
    pub memory: Option<u32>, // RAM in MB
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModLoader {
    Forge,
    Fabric,
    Quilt,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mod {
    pub id: String,
    pub name: String,
    pub version: String,
    pub source: ModSource,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModSource {
    CurseForge,
    Modrinth,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Theme {
    Light,
    Dark,
    System,
}

pub fn get_config_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "mosaic", "launcher")
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

    let config_dir = proj_dirs.config_dir();

    if !config_dir.exists() {
        fs::create_dir_all(config_dir)?;
        info!("Created config directory at {:?}", config_dir);
    }

    Ok(config_dir.to_path_buf())
}

pub fn get_config_file() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("config.json"))
}

// Define the old config format for migration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldConfig {
    pub minecraft_directory: PathBuf,
    pub profiles: Vec<Profile>,
    pub last_used_profile: Option<String>,
    pub theme: Theme,
    pub max_memory: u32, // in MB
    pub java_arguments: Vec<String>,
    pub java_path: Option<PathBuf>,
    pub disable_sandbox: bool, // Disable sandbox mode for games
}

// Function to migrate from old config format to new format
fn migrate_config(old_config: OldConfig) -> Config {
    info!("Migrating configuration from old format to new format");

    // Create a Minecraft game with the old minecraft_directory and profiles
    let minecraft_game = Game {
        id: "minecraft".to_string(),
        name: "Minecraft".to_string(),
        icon: None,
        executable: None,
        game_directory: old_config.minecraft_directory,
        profiles: old_config.profiles,
        game_type: GameType::Minecraft,
    };

    // Create a new config with the migrated data
    Config {
        games: vec![minecraft_game],
        selected_game: Some("minecraft".to_string()),
        last_used_profile: old_config.last_used_profile,
        theme: old_config.theme,
        max_memory: old_config.max_memory,
        java_arguments: old_config.java_arguments,
        java_path: old_config.java_path,
        disable_sandbox: old_config.disable_sandbox,
    }
}

pub fn load_config() -> Result<Config> {
    let config_file = get_config_file()?;

    if !config_file.exists() {
        warn!("Config file does not exist, creating default config");
        let default_config = default_config();
        save_config(&default_config)?;
        return Ok(default_config);
    }

    let config_clone = config_file.clone();
    let config_str = fs::read_to_string(config_file)?;

    // Try to deserialize into the new Config format
    match serde_json::from_str::<Config>(&config_str) {
        Ok(config) => {
            info!("Loaded configuration from {:?}", config_clone);
            Ok(config)
        },
        Err(e) => {
            // If that fails, try to deserialize into the old format and migrate
            warn!("Failed to load config in new format: {}. Attempting to migrate from old format.", e);
            match serde_json::from_str::<OldConfig>(&config_str) {
                Ok(old_config) => {
                    // Migrate from old format to new format
                    let config = migrate_config(old_config);

                    // Save the migrated config
                    save_config(&config)?;

                    info!("Successfully migrated and saved configuration");
                    Ok(config)
                },
                Err(e2) => {
                    // If both formats fail, return the original error
                    error!("Failed to load config in old format as well: {}", e2);
                    Err(anyhow::anyhow!("Failed to load configuration: {}", e))
                }
            }
        }
    }
}

pub fn save_config(config: &Config) -> Result<()> {
    let config_file = get_config_file()?;
    let config_str = serde_json::to_string_pretty(config)?;

    fs::write(config_file.clone(), config_str)?;
    info!("Saved configuration to {:?}", config_file);

    Ok(())
}

pub fn default_config() -> Config {
    let minecraft_dir = if cfg!(target_os = "windows") {
        PathBuf::from(format!("{}/.minecraft", std::env::var("APPDATA").unwrap_or_else(|_| String::from("."))))
    } else if cfg!(target_os = "macos") {
        PathBuf::from(format!("{}/Library/Application Support/minecraft", std::env::var("HOME").unwrap_or_else(|_| String::from("."))))
    } else {
        PathBuf::from(format!("{}/.minecraft", std::env::var("HOME").unwrap_or_else(|_| String::from("."))))
    };

    // Create a default Minecraft game
    let minecraft_game = Game {
        id: "minecraft".to_string(),
        name: "Minecraft".to_string(),
        icon: None,
        executable: None,
        game_directory: minecraft_dir,
        profiles: vec![],
        game_type: GameType::Minecraft,
    };

    Config {
        games: vec![minecraft_game],
        selected_game: Some("minecraft".to_string()),
        last_used_profile: None,
        theme: Theme::System,
        max_memory: 2048, // 2GB default
        java_arguments: vec!["-XX:+UseG1GC".to_string(), "-XX:+ParallelRefProcEnabled".to_string()],
        java_path: None,
        disable_sandbox: false, // Default to using sandbox mode
    }
}
