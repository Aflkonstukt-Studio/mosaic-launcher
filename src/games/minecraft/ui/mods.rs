// Minecraft mods manager for the Minecraft game plugin

use anyhow::{Result, anyhow};
use reqwest::Client as HttpClient;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::rc::Rc;
use tokio::fs;
use log::{info, warn, error, debug};

use crate::config::{Config, Profile, Mod, ModSource, ModLoader};
use crate::file_manager::{FileManager, DownloadProgress};
use crate::mods::{DependencyType, ModDependency, ModSearchParams, ModSearchResult, ModSortField, ModVersionInfo, SortOrder};

// API endpoints
const CURSEFORGE_API_BASE: &str = "https://api.curseforge.com/v1";
const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";

// You would need to register for a CurseForge API key
const CURSEFORGE_API_KEY: &str = "your-curseforge-api-key-here";

// Minecraft game ID for CurseForge API
const MINECRAFT_GAME_ID: u32 = 432;

// Mod loaders class IDs for CurseForge API
const FORGE_MODLOADER_ID: u32 = 6;
const FABRIC_MODLOADER_ID: u32 = 4;
const QUILT_MODLOADER_ID: u32 = 5;

// CurseForge API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeResponse<T> {
    data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeSearchResponse {
    pagination: CurseForgePagination,
    #[serde(rename = "data")]
    mods: Vec<CurseForgeMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgePagination {
    index: u32,
    pageSize: u32,
    totalCount: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeMod {
    id: u32,
    name: String,
    summary: String,
    downloadCount: u64,
    categories: Vec<CurseForgeCategory>,
    authors: Vec<CurseForgeAuthor>,
    logo: Option<CurseForgeAsset>,
    links: CurseForgeLinks,
    latestFiles: Vec<CurseForgeFile>,
    dateCreated: String,
    dateModified: String,
    gameId: u32,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeCategory {
    id: u32,
    name: String,
    slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeAuthor {
    id: u32,
    name: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeAsset {
    id: u32,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeLinks {
    websiteUrl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeFile {
    id: u32,
    fileName: String,
    displayName: String,
    fileDate: String,
    fileLength: u64,
    downloadUrl: String,
    gameVersions: Vec<String>,
    dependencies: Vec<CurseForgeDependency>,
    hashes: Vec<CurseForgeHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeDependency {
    modId: u32,
    relationType: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeHash {
    value: String,
    algo: u32,
}

// Modrinth API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthMod>,
    offset: u32,
    limit: u32,
    total_hits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthMod {
    slug: String,
    title: String,
    description: String,
    categories: Vec<String>,
    client_side: String,
    server_side: String,
    downloads: u64,
    follows: u64,
    versions: Vec<String>,
    icon_url: Option<String>,
    author: String,
    project_id: String,
    project_type: String,
    latest_version: String,
    date_created: String,
    date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthVersion {
    id: String,
    project_id: String,
    author_id: String,
    featured: bool,
    name: String,
    version_number: String,
    changelog: Option<String>,
    dependencies: Vec<ModrinthDependency>,
    game_versions: Vec<String>,
    version_type: String,
    loaders: Vec<String>,
    downloads: u64,
    files: Vec<ModrinthFile>,
    date_published: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthDependency {
    version_id: Option<String>,
    project_id: String,
    dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthFile {
    hashes: HashMap<String, String>,
    url: String,
    filename: String,
    primary: bool,
    size: u64,
}

pub struct MinecraftModManager {
    http_client: HttpClient,
    file_manager: FileManager,
    config: Config,
}

impl MinecraftModManager {
    pub fn new(config: Config, file_manager: Rc<FileManager>) -> Self {
        // Create HTTP client with CurseForge API key
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static(CURSEFORGE_API_KEY));

        let http_client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| HttpClient::new());

        Self {
            http_client,
            file_manager: (*file_manager).clone(),
            config,
        }
    }

    // Helper method to get the Minecraft directory from the selected game
    fn get_minecraft_directory(&self) -> PathBuf {
        // Get the selected game ID
        let selected_game_id = self.config.selected_game.clone().unwrap_or_else(|| {
            if !self.config.games.is_empty() {
                self.config.games[0].id.clone()
            } else {
                "minecraft".to_string()
            }
        });

        // Find the selected game
        let game = self.config.games.iter()
            .find(|g| g.id == selected_game_id)
            .unwrap_or_else(|| {
                // If the selected game doesn't exist, use the first game or create a default one
                if !self.config.games.is_empty() {
                    &self.config.games[0]
                } else {
                    panic!("No games found in config")
                }
            });

        game.game_directory.clone()
    }

    pub async fn search_mods(&self, params: &ModSearchParams) -> Result<Vec<ModSearchResult>> {
        info!("Searching for mods with query: {}", params.query);

        // Search on both platforms and combine results
        let mut results = Vec::new();

        // Search on CurseForge
        match self.search_curseforge(params).await {
            Ok(cf_results) => results.extend(cf_results),
            Err(e) => warn!("Error searching CurseForge: {}", e),
        }

        // Search on Modrinth
        match self.search_modrinth(params).await {
            Ok(mr_results) => results.extend(mr_results),
            Err(e) => warn!("Error searching Modrinth: {}", e),
        }

        // Sort results according to the requested sort field
        self.sort_results(&mut results, &params.sort_by, &params.sort_order);

        // Apply limit and offset
        let start = params.offset as usize;
        let end = (params.offset + params.limit) as usize;

        let results = results.into_iter()
            .skip(start)
            .take(end - start)
            .collect();

        Ok(results)
    }

    async fn search_curseforge(&self, params: &ModSearchParams) -> Result<Vec<ModSearchResult>> {
        // Convert ModLoader to CurseForge class ID
        let class_id = match &params.mod_loader {
            Some(ModLoader::Forge) => Some(FORGE_MODLOADER_ID),
            Some(ModLoader::Fabric) => Some(FABRIC_MODLOADER_ID),
            Some(ModLoader::Quilt) => Some(QUILT_MODLOADER_ID),
            _ => None,
        };

        // Build search URL
        let mut url = format!("{}/mods/search", CURSEFORGE_API_BASE);
        let mut query_params = vec![
            ("gameId", MINECRAFT_GAME_ID.to_string()),
            ("searchFilter", params.query.clone()),
            ("pageSize", params.limit.to_string()),
            ("index", params.offset.to_string()),
        ];

        // Add game version filter
        if !params.minecraft_version.is_empty() {
            query_params.push(("gameVersion", params.minecraft_version.clone()));
        }

        // Add mod loader filter
        if let Some(id) = class_id {
            query_params.push(("classId", id.to_string()));
        }

        // Add sort field
        let sort_field = match params.sort_by {
            ModSortField::Featured => 1,
            ModSortField::Popularity => 2,
            ModSortField::LastUpdated => 3,
            ModSortField::Name => 4,
            ModSortField::Author => 5,
            ModSortField::TotalDownloads => 6,
            ModSortField::Category => 7,
        };
        query_params.push(("sortField", sort_field.to_string()));

        // Add sort order
        let sort_order = match params.sort_order {
            SortOrder::Ascending => "asc",
            SortOrder::Descending => "desc",
        };
        query_params.push(("sortOrder", sort_order.to_string()));

        // Make the request
        let response = self.http_client.get(&url)
            .query(&query_params)
            .send()
            .await?
            .json::<CurseForgeResponse<CurseForgeSearchResponse>>()
            .await?;

        // Convert to our generic format
        let mut results = Vec::new();
        for cf_mod in response.data.mods {
            // Get the latest file for the specified Minecraft version
            let mut latest_file: Option<&CurseForgeFile> = None;
            for file in &cf_mod.latestFiles {
                if file.gameVersions.contains(&params.minecraft_version) {
                    if let Some(current) = &latest_file {
                        if file.fileDate > current.fileDate {
                            latest_file = Some(file);
                        }
                    } else {
                        latest_file = Some(file);
                    }
                }
            }

            // Skip if no compatible file found
            if latest_file.is_none() {
                continue;
            }

            let latest_file = latest_file.unwrap();

            // Convert dependencies
            let dependencies = latest_file.dependencies.iter()
                .map(|dep| ModDependency {
                    mod_id: dep.modId.to_string(),
                    version_id: None,
                    dependency_type: match dep.relationType {
                        1 => DependencyType::Required,
                        2 => DependencyType::Optional,
                        3 => DependencyType::Incompatible,
                        _ => DependencyType::Optional,
                    },
                })
                .collect();

            // Get SHA1 hash if available
            let sha1_hash = latest_file.hashes.iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone());

            // Convert to our format
            let mod_version = ModVersionInfo {
                id: latest_file.id.to_string(),
                name: latest_file.displayName.clone(),
                version_number: latest_file.fileName.clone(),
                game_versions: latest_file.gameVersions.clone(),
                mod_loaders: vec![],  // CurseForge doesn't provide this directly
                download_url: latest_file.downloadUrl.clone(),
                file_name: latest_file.fileName.clone(),
                file_size: latest_file.fileLength,
                release_date: latest_file.fileDate.clone(),
                dependencies,
                sha1_hash,
            };

            // Get the author name
            let author = if !cf_mod.authors.is_empty() {
                cf_mod.authors[0].name.clone()
            } else {
                "Unknown".to_string()
            };

            // Get the icon URL
            let icon_url = cf_mod.logo.as_ref().map(|logo| logo.url.clone());

            // Get categories
            let categories = cf_mod.categories.iter()
                .map(|cat| cat.name.clone())
                .collect();

            results.push(ModSearchResult {
                source: ModSource::CurseForge,
                id: cf_mod.id.to_string(),
                name: cf_mod.name,
                summary: cf_mod.summary,
                description: None,
                author,
                download_count: cf_mod.downloadCount,
                follows: None,
                icon_url,
                page_url: cf_mod.links.websiteUrl,
                versions: vec![mod_version.clone()],
                categories,
                latest_version: mod_version.version_number,
                latest_game_version: params.minecraft_version.clone(),
            });
        }

        Ok(results)
    }

    async fn search_modrinth(&self, params: &ModSearchParams) -> Result<Vec<ModSearchResult>> {
        // Build search URL
        let url = format!("{}/search", MODRINTH_API_BASE);

        // Convert ModLoader to Modrinth format
        let loader = match &params.mod_loader {
            Some(ModLoader::Forge) => Some("forge"),
            Some(ModLoader::Fabric) => Some("fabric"),
            Some(ModLoader::Quilt) => Some("quilt"),
            _ => None,
        };

        // Build facets for filtering
        let mut facets = Vec::new();

        // Add version facet
        if !params.minecraft_version.is_empty() {
            facets.push(format!("[\"versions:{}\"]", params.minecraft_version));
        }

        // Add loader facet
        if let Some(l) = loader {
            facets.push(format!("[\"categories:{}\"]", l));
        }

        // Add category facet
        if let Some(category) = &params.category {
            facets.push(format!("[\"categories:{}\"]", category));
        }

        // Combine facets
        let facets_str = if !facets.is_empty() {
            format!("[{}]", facets.join(","))
        } else {
            "[]".to_string()
        };

        // Convert sort field
        let sort = match params.sort_by {
            ModSortField::Featured => "relevance".to_string(),
            ModSortField::Popularity => "downloads".to_string(),
            ModSortField::LastUpdated => "updated".to_string(),
            ModSortField::Name => "name".to_string(),
            ModSortField::Author => "author".to_string(),
            ModSortField::TotalDownloads => "downloads".to_string(),
            ModSortField::Category => "relevance".to_string(),
        };

        // Make the request
        let response = self.http_client.get(&url)
            .query(&[
                ("query", &params.query),
                ("limit", &params.limit.to_string()),
                ("offset", &params.offset.to_string()),
                ("facets", &facets_str),
                ("sort", &sort),
            ])
            .send()
            .await?
            .json::<ModrinthSearchResponse>()
            .await?;

        // Convert to our generic format
        let mut results = Vec::new();
        for mr_mod in response.hits {
            // Fetch version details
            let mut versions = Vec::new();
            for version_id in &mr_mod.versions {
                if let Ok(version) = self.get_modrinth_version(version_id).await {
                    // Check if this version is compatible with the requested Minecraft version
                    if version.game_versions.contains(&params.minecraft_version) {
                        // Convert mod loaders
                        let mod_loaders = version.loaders.iter()
                            .filter_map(|loader| {
                                match loader.as_str() {
                                    "forge" => Some(ModLoader::Forge),
                                    "fabric" => Some(ModLoader::Fabric),
                                    "quilt" => Some(ModLoader::Quilt),
                                    _ => None,
                                }
                            })
                            .collect::<Vec<_>>();

                        // Skip if mod loader filter is set and this version doesn't match
                        if let Some(loader) = &params.mod_loader {
                            if !mod_loaders.contains(loader) {
                                continue;
                            }
                        }

                        // Get the primary file
                        if let Some(file) = version.files.iter().find(|f| f.primary) {
                            // Convert dependencies
                            let dependencies = version.dependencies.iter()
                                .map(|dep| ModDependency {
                                    mod_id: dep.project_id.clone(),
                                    version_id: dep.version_id.clone(),
                                    dependency_type: match dep.dependency_type.as_str() {
                                        "required" => DependencyType::Required,
                                        "optional" => DependencyType::Optional,
                                        "incompatible" => DependencyType::Incompatible,
                                        "embedded" => DependencyType::Embedded,
                                        _ => DependencyType::Optional,
                                    },
                                })
                                .collect();

                            // Get SHA1 hash if available
                            let sha1_hash = file.hashes.get("sha1").cloned();

                            versions.push(ModVersionInfo {
                                id: version.id.clone(),
                                name: version.name.clone(),
                                version_number: version.version_number.clone(),
                                game_versions: version.game_versions.clone(),
                                mod_loaders,
                                download_url: file.url.clone(),
                                file_name: file.filename.clone(),
                                file_size: file.size,
                                release_date: version.date_published.clone(),
                                dependencies,
                                sha1_hash,
                            });
                        }
                    }
                }
            }

            // Skip if no compatible versions found
            if versions.is_empty() {
                continue;
            }

            // Sort versions by release date (newest first)
            versions.sort_by(|a, b| b.release_date.cmp(&a.release_date));

            // Get the latest version
            let latest_version = versions.first().unwrap();

            results.push(ModSearchResult {
                source: ModSource::Modrinth,
                id: mr_mod.project_id,
                name: mr_mod.title,
                summary: mr_mod.description,
                description: None,
                author: mr_mod.author,
                download_count: mr_mod.downloads,
                follows: Some(mr_mod.follows),
                icon_url: mr_mod.icon_url,
                page_url: format!("https://modrinth.com/mod/{}", mr_mod.slug),
                versions: versions.clone(),
                categories: mr_mod.categories,
                latest_version: latest_version.version_number.clone(),
                latest_game_version: params.minecraft_version.clone(),
            });
        }

        Ok(results)
    }

    async fn get_modrinth_version(&self, version_id: &str) -> Result<ModrinthVersion> {
        let url = format!("{}/version/{}", MODRINTH_API_BASE, version_id);
        let response = self.http_client.get(&url)
            .send()
            .await?
            .json::<ModrinthVersion>()
            .await?;
        Ok(response)
    }

    fn sort_results(&self, results: &mut Vec<ModSearchResult>, sort_by: &ModSortField, sort_order: &SortOrder) {
        match sort_by {
            ModSortField::Name => {
                results.sort_by(|a, b| {
                    match sort_order {
                        SortOrder::Ascending => a.name.cmp(&b.name),
                        SortOrder::Descending => b.name.cmp(&a.name),
                    }
                });
            },
            ModSortField::Author => {
                results.sort_by(|a, b| {
                    match sort_order {
                        SortOrder::Ascending => a.author.cmp(&b.author),
                        SortOrder::Descending => b.author.cmp(&a.author),
                    }
                });
            },
            ModSortField::TotalDownloads => {
                results.sort_by(|a, b| {
                    match sort_order {
                        SortOrder::Ascending => a.download_count.cmp(&b.download_count),
                        SortOrder::Descending => b.download_count.cmp(&a.download_count),
                    }
                });
            },
            ModSortField::LastUpdated => {
                results.sort_by(|a, b| {
                    if let (Some(a_version), Some(b_version)) = (a.versions.first(), b.versions.first()) {
                        match sort_order {
                            SortOrder::Ascending => a_version.release_date.cmp(&b_version.release_date),
                            SortOrder::Descending => b_version.release_date.cmp(&a_version.release_date),
                        }
                    } else {
                        std::cmp::Ordering::Equal
                    }
                });
            },
            // For other sort fields, we'll just use the order they came in
            _ => {}
        }
    }

    pub async fn get_mod_details(&self, mod_id: &str, source: &ModSource) -> Result<ModSearchResult> {
        match source {
            ModSource::CurseForge => self.get_curseforge_mod_details(mod_id).await,
            ModSource::Modrinth => self.get_modrinth_mod_details(mod_id).await,
            ModSource::Manual => Err(anyhow!("Cannot get details for manually installed mods")),
        }
    }

    async fn get_curseforge_mod_details(&self, mod_id: &str) -> Result<ModSearchResult> {
        let url = format!("{}/mods/{}", CURSEFORGE_API_BASE, mod_id);
        let response = self.http_client.get(&url)
            .send()
            .await?
            .json::<CurseForgeResponse<CurseForgeMod>>()
            .await?;

        let cf_mod = response.data;

        // Get the author name
        let author = if !cf_mod.authors.is_empty() {
            cf_mod.authors[0].name.clone()
        } else {
            "Unknown".to_string()
        };

        // Get the icon URL
        let icon_url = cf_mod.logo.as_ref().map(|logo| logo.url.clone());

        // Get categories
        let categories = cf_mod.categories.iter()
            .map(|cat| cat.name.clone())
            .collect();

        // Convert files to versions
        let mut versions = Vec::new();
        for file in &cf_mod.latestFiles {
            // Convert dependencies
            let dependencies = file.dependencies.iter()
                .map(|dep| ModDependency {
                    mod_id: dep.modId.to_string(),
                    version_id: None,
                    dependency_type: match dep.relationType {
                        1 => DependencyType::Required,
                        2 => DependencyType::Optional,
                        3 => DependencyType::Incompatible,
                        _ => DependencyType::Optional,
                    },
                })
                .collect();

            // Get SHA1 hash if available
            let sha1_hash = file.hashes.iter()
                .find(|hash| hash.algo == 1)
                .map(|hash| hash.value.clone());

            versions.push(ModVersionInfo {
                id: file.id.to_string(),
                name: file.displayName.clone(),
                version_number: file.fileName.clone(),
                game_versions: file.gameVersions.clone(),
                mod_loaders: vec![],  // CurseForge doesn't provide this directly
                download_url: file.downloadUrl.clone(),
                file_name: file.fileName.clone(),
                file_size: file.fileLength,
                release_date: file.fileDate.clone(),
                dependencies,
                sha1_hash,
            });
        }

        // Sort versions by release date (newest first)
        versions.sort_by(|a, b| b.release_date.cmp(&a.release_date));

        // Get the latest version and game version
        let latest_version = if !versions.is_empty() {
            versions[0].version_number.clone()
        } else {
            "Unknown".to_string()
        };

        let latest_game_version = if !versions.is_empty() && !versions[0].game_versions.is_empty() {
            versions[0].game_versions[0].clone()
        } else {
            "Unknown".to_string()
        };

        Ok(ModSearchResult {
            source: ModSource::CurseForge,
            id: cf_mod.id.to_string(),
            name: cf_mod.name,
            summary: cf_mod.summary,
            description: None,
            author,
            download_count: cf_mod.downloadCount,
            follows: None,
            icon_url,
            page_url: cf_mod.links.websiteUrl,
            versions,
            categories,
            latest_version,
            latest_game_version,
        })
    }

    async fn get_modrinth_mod_details(&self, mod_id: &str) -> Result<ModSearchResult> {
        // Get project details
        let project_url = format!("{}/project/{}", MODRINTH_API_BASE, mod_id);
        let project = self.http_client.get(&project_url)
            .send()
            .await?
            .json::<ModrinthMod>()
            .await?;

        // Get version details
        let versions_url = format!("{}/project/{}/version", MODRINTH_API_BASE, mod_id);
        let versions_response = self.http_client.get(&versions_url)
            .send()
            .await?
            .json::<Vec<ModrinthVersion>>()
            .await?;

        // Convert versions
        let mut versions = Vec::new();
        for version in versions_response {
            // Get the primary file
            if let Some(file) = version.files.iter().find(|f| f.primary) {
                // Convert dependencies
                let dependencies = version.dependencies.iter()
                    .map(|dep| ModDependency {
                        mod_id: dep.project_id.clone(),
                        version_id: dep.version_id.clone(),
                        dependency_type: match dep.dependency_type.as_str() {
                            "required" => DependencyType::Required,
                            "optional" => DependencyType::Optional,
                            "incompatible" => DependencyType::Incompatible,
                            "embedded" => DependencyType::Embedded,
                            _ => DependencyType::Optional,
                        },
                    })
                    .collect();

                // Get SHA1 hash if available
                let sha1_hash = file.hashes.get("sha1").cloned();

                // Convert mod loaders
                let mod_loaders = version.loaders.iter()
                    .filter_map(|loader| {
                        match loader.as_str() {
                            "forge" => Some(ModLoader::Forge),
                            "fabric" => Some(ModLoader::Fabric),
                            "quilt" => Some(ModLoader::Quilt),
                            _ => None,
                        }
                    })
                    .collect();

                versions.push(ModVersionInfo {
                    id: version.id.clone(),
                    name: version.name.clone(),
                    version_number: version.version_number.clone(),
                    game_versions: version.game_versions.clone(),
                    mod_loaders,
                    download_url: file.url.clone(),
                    file_name: file.filename.clone(),
                    file_size: file.size,
                    release_date: version.date_published.clone(),
                    dependencies,
                    sha1_hash,
                });
            }
        }

        // Sort versions by release date (newest first)
        versions.sort_by(|a, b| b.release_date.cmp(&a.release_date));

        // Get the latest version and game version
        let latest_version = if !versions.is_empty() {
            versions[0].version_number.clone()
        } else {
            "Unknown".to_string()
        };

        let latest_game_version = if !versions.is_empty() && !versions[0].game_versions.is_empty() {
            versions[0].game_versions[0].clone()
        } else {
            "Unknown".to_string()
        };

        Ok(ModSearchResult {
            source: ModSource::Modrinth,
            id: project.project_id,
            name: project.title,
            summary: project.description,
            description: None,
            author: project.author,
            download_count: project.downloads,
            follows: Some(project.follows),
            icon_url: project.icon_url,
            page_url: format!("https://modrinth.com/mod/{}", project.slug),
            versions,
            categories: project.categories,
            latest_version,
            latest_game_version,
        })
    }

    pub async fn install_mod(&self, mod_version: &ModVersionInfo, profile: &Profile) -> Result<()> {
        // Get the Minecraft directory
        let minecraft_dir = self.get_minecraft_directory();

        // Get the mods directory for this profile
        let mods_dir = minecraft_dir.join("mods");

        // Create the mods directory if it doesn't exist
        if !mods_dir.exists() {
            fs::create_dir_all(&mods_dir).await?;
        }

        // Download the mod file
        let mod_path = mods_dir.join(&mod_version.file_name);
        let expected_hash = mod_version.sha1_hash.as_deref();
        self.file_manager.download_file(&mod_version.download_url, &mod_path, expected_hash, |_| {}).await?;

        Ok(())
    }

    pub async fn uninstall_mod(&self, mod_file: &str, profile: &Profile) -> Result<()> {
        // Get the Minecraft directory
        let minecraft_dir = self.get_minecraft_directory();

        // Get the mods directory for this profile
        let mods_dir = minecraft_dir.join("mods");

        // Check if the mod file exists
        let mod_path = mods_dir.join(mod_file);
        if !mod_path.exists() {
            return Err(anyhow!("Mod file {} does not exist", mod_file));
        }

        // Delete the mod file
        fs::remove_file(&mod_path).await?;

        Ok(())
    }

    pub async fn get_installed_mods(&self, profile: &Profile) -> Result<Vec<Mod>> {
        // Get the Minecraft directory
        let minecraft_dir = self.get_minecraft_directory();

        // Get the mods directory for this profile
        let mods_dir = minecraft_dir.join("mods");

        // Check if the mods directory exists
        if !mods_dir.exists() {
            return Ok(vec![]);
        }

        // Read the directory
        let mut entries = fs::read_dir(&mods_dir).await?;
        let mut mods = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "jar") {
                // Get the file name
                let file_name = path.file_name().unwrap().to_string_lossy().to_string();

                // Get the file size
                let metadata = fs::metadata(&path).await?;
                let file_size = metadata.len();

                // Add to the list
                mods.push(Mod {
                    id: file_name.clone(),
                    name: file_name.clone(),
                    version: "None".to_string(),
                    source: ModSource::Manual,
                    enabled: true,
                });
            }
        }

        Ok(mods)
    }
}
