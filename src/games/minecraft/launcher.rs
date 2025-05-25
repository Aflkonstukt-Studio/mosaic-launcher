// Launcher for Minecraft

use anyhow::{Result, anyhow};
use log::{info, warn, error, debug};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, Child};
use std::collections::HashMap;

use crate::config::Profile;
use crate::auth::AuthSession;
use super::models::{VersionDetails, Arguments};
use super::versions;

/// Launches Minecraft with the specified profile and authentication session
pub async fn launch_game(
    minecraft_dir: &Path,
    profile: &Profile,
    auth_session: &AuthSession,
    version_details: &VersionDetails,
    java_path: &Path,
) -> Result<u32> {
    info!("Launching Minecraft with profile: {}", profile.name);

    // Build the command to launch Minecraft
    let mut command = Command::new(java_path);

    // Set the working directory to the game directory or the .minecraft directory
    let game_dir = if let Some(dir) = &profile.game_directory {
        PathBuf::from(dir)
    } else {
        minecraft_dir.to_path_buf()
    };

    command.current_dir(&game_dir);

    // Add memory settings
    let memory = profile.memory.unwrap_or(2048);
    command.arg(format!("-Xmx{}M", memory));
    command.arg(format!("-Xms{}M", memory));

    // Add standard JVM arguments
    command.arg("-XX:+UnlockExperimentalVMOptions");
    command.arg("-XX:+UseG1GC");
    command.arg("-XX:G1NewSizePercent=20");
    command.arg("-XX:G1ReservePercent=20");
    command.arg("-XX:MaxGCPauseMillis=50");
    command.arg("-XX:G1HeapRegionSize=32M");
    command.arg("-Dfile.encoding=UTF-8");
    command.arg("-Djava.library.path=natives");
    command.arg("-Dminecraft.launcher.brand=MosaicLauncher");
    command.arg("-Dminecraft.launcher.version=1.0.0");
    command.arg("-Dorg.lwjgl.util.DebugLoader=true");

    // Build classpath
    let classpath = versions::build_classpath(minecraft_dir, version_details)?;
    command.arg("-cp").arg(classpath);

    // Determine main class based on modloader
    let main_class = if let Some(mod_loader) = &profile.mod_loader {
        match mod_loader {
            crate::config::ModLoader::Forge => {
                // For Forge, we need to use the forge main class
                // The exact class name depends on the Forge version
                let forge_version = profile.mod_loader_version.clone().unwrap_or_else(|| {
                    // Get a known version for this Minecraft version
                    match version_details.id.as_str() {
                        "1.21.5" => "1.21.5-47.1.0".to_string(),
                        "1.21.1" => "1.21.1-47.0.1".to_string(),
                        "1.20.4" => "1.20.4-49.0.3".to_string(),
                        "1.20.1" => "1.20.1-47.2.0".to_string(),
                        "1.19.4" => "1.19.4-45.1.0".to_string(),
                        "1.19.2" => "1.19.2-43.2.0".to_string(),
                        "1.18.2" => "1.18.2-40.2.0".to_string(),
                        "1.17.1" => "1.17.1-37.1.1".to_string(),
                        "1.16.5" => "1.16.5-36.2.39".to_string(),
                        _ => {
                            warn!("No known Forge version for Minecraft {}, using a placeholder", version_details.id);
                            format!("{}-unknown", version_details.id)
                        }
                    }
                });

                // Add Forge libraries to classpath
                // This is handled by the build_classpath function

                // Return the Forge main class
                "net.minecraftforge.client.main.Main"
            },
            crate::config::ModLoader::NeoForge => {
                // For NeoForge, we need to use the NeoForge main class
                // The exact class name depends on the NeoForge version
                let neoforge_version = profile.mod_loader_version.clone().unwrap_or_else(|| {
                    // Get a known version for this Minecraft version
                    match version_details.id.as_str() {
                        "1.21.1" => "1.21.1-47.0.1".to_string(),
                        "1.20.4" => "1.20.4-49.0.3".to_string(),
                        "1.20.1" => "1.20.1-47.2.0".to_string(),
                        "1.19.4" => "1.19.4-45.1.0".to_string(),
                        "1.19.2" => "1.19.2-43.2.0".to_string(),
                        _ => {
                            warn!("No known NeoForge version for Minecraft {}, using a placeholder", version_details.id);
                            format!("{}-unknown", version_details.id)
                        }
                    }
                });

                // Add NeoForge libraries to classpath
                // This is handled by the build_classpath function

                // Return the NeoForge main class
                "net.neoforged.client.main.Main"
            },
            crate::config::ModLoader::Fabric => {
                // For Fabric, we need to use the Fabric main class
                // Add Fabric libraries to classpath
                // This is handled by the build_classpath function

                // Return the Fabric main class
                "net.fabricmc.loader.impl.launch.knot.KnotClient"
            },
            crate::config::ModLoader::Quilt => {
                // For Quilt, we need to use the Quilt main class
                // Add Quilt libraries to classpath
                // This is handled by the build_classpath function

                // Return the Quilt main class
                "org.quiltmc.loader.impl.launch.knot.KnotClient"
            },
            crate::config::ModLoader::None => {
                // For vanilla, use the default main class
                version_details.main_class.as_deref().unwrap_or("net.minecraft.client.main.Main")
            }
        }
    } else {
        // No modloader specified, use the default main class
        version_details.main_class.as_deref().unwrap_or("net.minecraft.client.main.Main")
    };

    command.arg(main_class);

    // Add game arguments
    add_game_arguments(&mut command, profile, auth_session, version_details, minecraft_dir)?;

    // Launch the game
    info!("Launching Minecraft with command: {:?}", command);
    let child = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    let pid = child.id();
    info!("Launched with PID: {}", pid);
    Ok(pid)
}

/// Adds game arguments to the command
fn add_game_arguments(
    command: &mut Command,
    profile: &Profile,
    auth_session: &AuthSession,
    version_details: &VersionDetails,
    minecraft_dir: &Path,
) -> Result<()> {
    // Get the Minecraft profile from the auth session
    let minecraft_profile = auth_session.minecraft_profile.as_ref()
        .ok_or_else(|| anyhow!("No Minecraft profile in auth session"))?;

    // Get the game directory
    let game_dir = if let Some(dir) = &profile.game_directory {
        PathBuf::from(dir)
    } else {
        minecraft_dir.to_path_buf()
    };

    // Get the assets directory
    let assets_dir = minecraft_dir.join("assets");

    // Get the assets index
    let assets_index = version_details.assets.clone();

    // Get the assets path
    let assets_path = if assets_index == "legacy" {
        minecraft_dir.join("assets").join("virtual").join("legacy").to_string_lossy().to_string()
    } else {
        minecraft_dir.join("assets").to_string_lossy().to_string()
    };

    // Add arguments from the version details
    if let Some(arguments) = &version_details.arguments {
        // Add game arguments
        for arg in &arguments.game {
            match arg {
                serde_json::Value::String(s) => {
                    let arg = replace_placeholders(
                        s,
                        minecraft_profile,
                        auth_session,
                        &version_details.id,
                        &game_dir,
                        &assets_dir,
                        &assets_index,
                        &assets_path,
                    )?;
                    command.arg(arg);
                }
                serde_json::Value::Object(obj) => {
                    // Check if the argument should be included
                    if should_include_argument(obj) {
                        if let Some(value) = obj.get("value") {
                            match value {
                                serde_json::Value::String(s) => {
                                    let arg = replace_placeholders(
                                        s,
                                        minecraft_profile,
                                        auth_session,
                                        &version_details.id,
                                        &game_dir,
                                        &assets_dir,
                                        &assets_index,
                                        &assets_path,
                                    )?;
                                    command.arg(arg);
                                }
                                serde_json::Value::Array(arr) => {
                                    for val in arr {
                                        if let serde_json::Value::String(s) = val {
                                            let arg = replace_placeholders(
                                                s,
                                                minecraft_profile,
                                                auth_session,
                                                &version_details.id,
                                                &game_dir,
                                                &assets_dir,
                                                &assets_index,
                                                &assets_path,
                                            )?;
                                            command.arg(arg);
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    } else if let Some(minecraft_arguments) = &version_details.minecraft_arguments {
        // For older versions, use minecraft_arguments
        let args = minecraft_arguments.split(' ');
        for arg in args {
            let arg = replace_placeholders(
                arg,
                minecraft_profile,
                auth_session,
                &version_details.id,
                &game_dir,
                &assets_dir,
                &assets_index,
                &assets_path,
            )?;
            command.arg(arg);
        }
    }

    Ok(())
}

/// Checks if an argument should be included based on rules
fn should_include_argument(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    // If there are no rules, the argument is always included
    if !obj.contains_key("rules") {
        return true;
    }

    let rules = match obj.get("rules") {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return true,
    };

    let mut allowed = false;

    for rule in rules {
        let rule_obj = match rule {
            serde_json::Value::Object(obj) => obj,
            _ => continue,
        };

        let action = match rule_obj.get("action") {
            Some(serde_json::Value::String(s)) => s.as_str(),
            _ => continue,
        };

        let action_allowed = action == "allow";

        // If there's no OS specified, the rule applies to all OSes
        if !rule_obj.contains_key("os") {
            allowed = action_allowed;
            continue;
        }

        let os = match rule_obj.get("os") {
            Some(serde_json::Value::Object(obj)) => obj,
            _ => continue,
        };

        // Check if the rule applies to the current OS
        let os_name = match os.get("name") {
            Some(serde_json::Value::String(s)) => s.as_str(),
            _ => continue,
        };

        let current_os = if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "osx"
        } else {
            "linux"
        };

        if os_name == current_os {
            allowed = action_allowed;
        }
    }

    allowed
}

/// Replaces placeholders in arguments
fn replace_placeholders(
    arg: &str,
    minecraft_profile: &crate::auth::MinecraftProfile,
    auth_session: &crate::auth::AuthSession,
    version_id: &str,
    game_dir: &Path,
    assets_dir: &Path,
    assets_index: &str,
    assets_path: &str,
) -> Result<String> {
    let arg = arg.replace("${auth_player_name}", &minecraft_profile.name)
        .replace("${version_name}", version_id)
        .replace("${game_directory}", &game_dir.to_string_lossy())
        .replace("${assets_root}", &assets_dir.to_string_lossy())
        .replace("${assets_index_name}", assets_index)
        .replace("${auth_uuid}", &minecraft_profile.id)
        .replace("${auth_access_token}", &auth_session.access_token)
        .replace("${user_type}", "msa")
        .replace("${version_type}", "release")
        .replace("${user_properties}", "{}")
        .replace("${auth_session}", &format!("token:{}", auth_session.access_token))
        .replace("${game_assets}", assets_path)
        .replace("${auth_xuid}", "");

    Ok(arg)
}

/// Gets the Java path
pub fn get_java_path() -> Result<PathBuf> {
    // Try to find Java in the system path
    let java_path = if cfg!(target_os = "windows") {
        which::which("javaw").or_else(|_| which::which("java"))
    } else {
        which::which("java")
    };

    match java_path {
        Ok(path) => {
            info!("Found Java at: {}", path.display());
            Ok(path)
        }
        Err(_) => {
            // Try common Java installation locations
            let common_paths = if cfg!(target_os = "windows") {
                vec![
                    "C:\\Program Files\\Java\\jre8\\bin\\javaw.exe",
                    "C:\\Program Files\\Java\\jre7\\bin\\javaw.exe",
                    "C:\\Program Files\\Java\\jre6\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jre8\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jre7\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jre6\\bin\\javaw.exe",
                    "C:\\Program Files\\Java\\jdk1.8.0\\bin\\javaw.exe",
                    "C:\\Program Files\\Java\\jdk1.7.0\\bin\\javaw.exe",
                    "C:\\Program Files\\Java\\jdk1.6.0\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jdk1.8.0\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jdk1.7.0\\bin\\javaw.exe",
                    "C:\\Program Files (x86)\\Java\\jdk1.6.0\\bin\\javaw.exe",
                ]
            } else if cfg!(target_os = "macos") {
                vec![
                    "/Library/Java/JavaVirtualMachines/jdk1.8.0.jdk/Contents/Home/bin/java",
                    "/Library/Java/JavaVirtualMachines/jdk1.7.0.jdk/Contents/Home/bin/java",
                    "/Library/Java/JavaVirtualMachines/jdk1.6.0.jdk/Contents/Home/bin/java",
                    "/System/Library/Java/JavaVirtualMachines/1.8.0.jdk/Contents/Home/bin/java",
                    "/System/Library/Java/JavaVirtualMachines/1.7.0.jdk/Contents/Home/bin/java",
                    "/System/Library/Java/JavaVirtualMachines/1.6.0.jdk/Contents/Home/bin/java",
                ]
            } else {
                vec![
                    "/usr/bin/java",
                    "/usr/local/bin/java",
                    "/usr/lib/jvm/default-java/bin/java",
                    "/usr/lib/jvm/java-8-openjdk/bin/java",
                    "/usr/lib/jvm/java-8-oracle/bin/java",
                    "/usr/lib/jvm/java-7-openjdk/bin/java",
                    "/usr/lib/jvm/java-7-oracle/bin/java",
                    "/usr/lib/jvm/java-6-openjdk/bin/java",
                    "/usr/lib/jvm/java-6-oracle/bin/java",
                ]
            };

            for path in common_paths {
                let path = PathBuf::from(path);
                if path.exists() {
                    info!("Found Java at: {}", path.display());
                    return Ok(path);
                }
            }

            Err(anyhow!("Java not found. Please install Java and make sure it's in your PATH."))
        }
    }
}

/// Validates the Java installation
pub fn validate_java() -> Result<String> {
    let java_path = get_java_path()?;

    // Run java -version and capture the output
    let output = Command::new(&java_path)
        .arg("-version")
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("Failed to run Java. Please make sure Java is installed correctly."));
    }

    // Parse the version from the output
    let version_output = String::from_utf8_lossy(&output.stderr);
    let version_line = version_output.lines().next().unwrap_or("");

    info!("Java found: {}", version_line);
    Ok(version_line.to_string())
}
