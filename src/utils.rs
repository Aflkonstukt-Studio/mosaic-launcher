use anyhow::Result;
use std::path::Path;
use std::fs;
use log::{info, warn, error, debug};

/// Ensures that a directory exists, creating it if necessary
pub fn ensure_dir_exists<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        fs::create_dir_all(path)?;
        info!("Created directory: {:?}", path);
    }
    Ok(())
}

/// Formats a file size in bytes to a human-readable string
pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

/// Formats a duration in milliseconds to a human-readable string
pub fn format_duration(millis: u64) -> String {
    let seconds = millis / 1000;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes % 60, seconds % 60)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds % 60)
    } else {
        format!("{}s", seconds)
    }
}

/// Extracts the file name from a path
pub fn get_file_name<P: AsRef<Path>>(path: P) -> Option<String> {
    path.as_ref()
        .file_name()
        .and_then(|name| name.to_str())
        .map(|s| s.to_string())
}

/// Extracts the file extension from a path
pub fn get_file_extension<P: AsRef<Path>>(path: P) -> Option<String> {
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_string())
}

/// Checks if a file has a specific extension
pub fn has_extension<P: AsRef<Path>>(path: P, extension: &str) -> bool {
    get_file_extension(path)
        .map(|ext| ext.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

/// Checks if a string is a valid Minecraft version
pub fn is_valid_minecraft_version(version: &str) -> bool {
    // Simple validation - could be more sophisticated
    !version.is_empty() && version.contains('.')
}

/// Checks if a string is a valid Java path
pub fn is_valid_java_path<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    
    if !path.exists() {
        return false;
    }
    
    if !path.is_file() {
        return false;
    }
    
    let file_name = match get_file_name(path) {
        Some(name) => name,
        None => return false,
    };
    
    #[cfg(target_os = "windows")]
    {
        file_name.eq_ignore_ascii_case("java.exe") || file_name.eq_ignore_ascii_case("javaw.exe")
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        file_name == "java"
    }
}

/// Sanitizes a file name by removing invalid characters
pub fn sanitize_file_name(name: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut result = String::with_capacity(name.len());
    
    for c in name.chars() {
        if !invalid_chars.contains(&c) {
            result.push(c);
        } else {
            result.push('_');
        }
    }
    
    result
}

/// Truncates a string to a maximum length, adding an ellipsis if truncated
pub fn truncate_string(s: &str, max_length: usize) -> String {
    if s.len() <= max_length {
        s.to_string()
    } else {
        format!("{}...", &s[0..max_length - 3])
    }
}

/// Converts a string to title case
pub fn to_title_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    
    for c in s.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            result.push(c);
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else {
            result.extend(c.to_lowercase());
        }
    }
    
    result
}