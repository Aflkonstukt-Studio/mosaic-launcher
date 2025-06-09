// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn microsoft_login() -> Result<serde_json::Value, String> {
    // Placeholder for Microsoft login logic
    // In a real implementation, this would handle OAuth flow
    Ok(serde_json::json!({
        "success": true,
        "auth_type": "microsoft",
        "username": "Player",
        "uuid": "00000000-0000-0000-0000-000000000000",
        "access_token": "mock_token_123"
    }))
}

#[tauri::command]
fn offline_login(username: String) -> Result<serde_json::Value, String> {
    // Validate username
    if username.trim().is_empty() {
        return Err("Username cannot be empty".to_string());
    }
    
    // Placeholder for offline login logic
    Ok(serde_json::json!({
        "success": true,
        "auth_type": "offline",
        "username": username.trim(),
        "uuid": format!("offline-{}", uuid::Uuid::new_v4()),
        "access_token": null
    }))
}

#[tauri::command]
fn get_windows_accent_color() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        
        // Try multiple registry sources for the accent color
        let registry_paths = [
            ("HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\DWM", "AccentColor"),
            ("HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\DWM", "ColorizationColor"),
            ("HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent", "AccentColorMenu"),
        ];
        
        for (path, value_name) in &registry_paths {
            let output = Command::new("reg")
                .args(&["query", path, "/v", value_name])
                .output();
                
            if let Ok(output) = output {
                let output_str = String::from_utf8_lossy(&output.stdout);
                
                for line in output_str.lines() {
                    if line.contains(value_name) && line.contains("REG_DWORD") {
                        if let Some(hex_part) = line.split_whitespace().last() {
                            if let Ok(color_value) = u32::from_str_radix(&hex_part.replace("0x", ""), 16) {
                                // Try different interpretations of the color format
                                
                                // Method 1: ABGR format (Windows DWM)
                                let b1 = (color_value >> 16) & 0xFF;
                                let g1 = (color_value >> 8) & 0xFF;
                                let r1 = color_value & 0xFF;
                                
                                // Method 2: ARGB format
                                let r2 = (color_value >> 16) & 0xFF;
                                let g2 = (color_value >> 8) & 0xFF;
                                let b2 = color_value & 0xFF;
                                
                                // Use the method that gives a more reasonable color (avoid too dark/bright)
                                let (r, g, b) = if *value_name == "ColorizationColor" {
                                    // ColorizationColor is usually ARGB
                                    (r2, g2, b2)
                                } else {
                                    // AccentColor is usually ABGR
                                    (r1, g1, b1)
                                };
                                
                                return Ok(format!("#{:02x}{:02x}{:02x}", r, g, b));
                            }
                        }
                    }
                }
            }
        }
        
        // Ultimate fallback - try PowerShell to get theme colors
        let ps_output = Command::new("powershell")
            .args(&["-Command", "Get-ItemProperty -Path 'HKCU:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent' -Name 'AccentColorMenu' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AccentColorMenu"])
            .output();
            
        if let Ok(ps_output) = ps_output {
            let ps_str = String::from_utf8_lossy(&ps_output.stdout).trim().to_string();
            if let Ok(color_value) = ps_str.parse::<u32>() {
                let r = (color_value >> 16) & 0xFF;
                let g = (color_value >> 8) & 0xFF;
                let b = color_value & 0xFF;
                return Ok(format!("#{:02x}{:02x}{:02x}", r, g, b));
            }
        }
        
        // Fallback to default Windows blue
        Ok("#0078D4".to_string())
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Fallback for non-Windows platforms
        Ok("#0078D4".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![microsoft_login, offline_login, get_windows_accent_color])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}