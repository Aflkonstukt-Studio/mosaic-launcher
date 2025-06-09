// Mods module for Mosaic Launcher
// This file defines the interfaces for mod management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;
use tokio::fs;
use log::{info, warn, error, debug};
use crate::config::{Config, Profile, Mod, ModSource, ModLoader};
use crate::file_manager::{FileManager, DownloadProgress};

/// Enum representing the mod sort field
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModSortField {
    Featured,
    Popularity,
    LastUpdated,
    Name,
    Author,
    TotalDownloads,
    Category,
}

/// Enum representing the sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Enum representing the dependency type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

/// Struct representing a mod dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependency {
    pub mod_id: String,
    pub version_id: Option<String>,
    pub dependency_type: DependencyType,
}

/// Struct representing a mod search parameters
#[derive(Debug, Clone)]
pub struct ModSearchParams {
    pub query: String,
    pub mod_loader: Option<ModLoader>,
    pub minecraft_version: String,
    pub category: Option<String>,
    pub sort_by: ModSortField,
    pub sort_order: SortOrder,
    pub limit: u32,
    pub offset: u32,
}

/// Struct representing a mod version info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModVersionInfo {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub mod_loaders: Vec<ModLoader>,
    pub download_url: String,
    pub file_name: String,
    pub file_size: u64,
    pub release_date: String,
    pub dependencies: Vec<ModDependency>,
    pub sha1_hash: Option<String>,
}

/// Struct representing a mod search result
#[derive(Debug, Clone)]
pub struct ModSearchResult {
    pub source: ModSource,
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: Option<String>,
    pub author: String,
    pub download_count: u64,
    pub follows: Option<u64>,
    pub icon_url: Option<String>,
    pub page_url: String,
    pub versions: Vec<ModVersionInfo>,
    pub categories: Vec<String>,
    pub latest_version: String,
    pub latest_game_version: String,
}

/// Trait for mod managers
pub trait ModManager {
    /// Search for mods
    fn search_mods(&self, params: &ModSearchParams) -> Result<Vec<ModSearchResult>>;

    /// Get mod details
    fn get_mod_details(&self, mod_id: &str, source: &ModSource) -> Result<ModSearchResult>;

    /// Install a mod
    fn install_mod(&self, mod_version: &ModVersionInfo, profile: &Profile) -> Result<()>;

    /// Uninstall a mod
    fn uninstall_mod(&self, mod_file: &str, profile: &Profile) -> Result<()>;

    /// Get installed mods
    fn get_installed_mods(&self, profile: &Profile) -> Result<Vec<Mod>>;
}