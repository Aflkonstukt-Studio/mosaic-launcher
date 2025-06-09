// Data models for the Minecraft game plugin

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Minecraft version manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionInfo>,
}

/// Latest Minecraft versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

/// Minecraft version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub r#type: String,
    pub url: String,
    pub time: String,
    #[serde(default)]
    pub release_time: String,
}

/// Minecraft version details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionDetails {
    pub id: String,
    pub r#type: String,
    pub time: String,
    #[serde(default)]
    pub release_time: String,
    pub main_class: Option<String>,
    pub minimum_launcher_version: Option<u32>,
    pub assets: String,
    pub assets_index: Option<AssetIndex>,
    pub downloads: HashMap<String, Download>,
    pub libraries: Vec<Library>,
    pub logging: Option<Logging>,
    pub arguments: Option<Arguments>,
    pub minecraft_arguments: Option<String>,
}

/// Asset index information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
    pub total_size: u64,
}

/// Download information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Library information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

/// Library downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Download>,
    pub classifiers: Option<HashMap<String, Download>>,
}

/// Rule for library inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<Os>,
    pub features: Option<HashMap<String, bool>>,
}

/// Operating system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Os {
    pub name: Option<String>,
    pub version: Option<String>,
    pub arch: Option<String>,
}

/// Extraction information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extract {
    pub exclude: Vec<String>,
}

/// Logging information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Logging {
    pub client: LoggingClient,
}

/// Logging client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
    pub r#type: String,
}

/// Logging file information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// Arguments information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

/// Asset objects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetObject>,
}

/// Asset object information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}