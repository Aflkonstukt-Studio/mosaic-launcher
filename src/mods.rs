use anyhow::{Result, anyhow};
use reqwest::Client as HttpClient;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::fs;
use log::{info, warn, error, debug};
use crate::config::{Config, Profile, Mod, ModSource, ModLoader};
use crate::file_manager::{FileManager, DownloadProgress};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModSortField {
    Featured,
    Popularity,
    LastUpdated,
    Name,
    Author,
    TotalDownloads,
    Category,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModDependency {
    pub mod_id: String,
    pub version_id: Option<String>,
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

// CurseForge API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeResponse<T> {
    data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeSearchResponse {
    pagination: CurseForgePagination,
    data: Vec<CurseForgeMod>,
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
    logo: Option<CurseForgeAttachment>,
    authors: Vec<CurseForgeAuthor>,
    latestFiles: Vec<CurseForgeFile>,
    dateCreated: String,
    dateModified: String,
    links: CurseForgeLinks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeCategory {
    id: u32,
    name: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeAttachment {
    id: u32,
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeAuthor {
    id: u32,
    name: String,
    url: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CurseForgeLinks {
    websiteUrl: String,
}

// Modrinth API response structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthSearchHit>,
    offset: u32,
    limit: u32,
    total_hits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthSearchHit {
    slug: String,
    title: String,
    description: String,
    categories: Vec<String>,
    client_side: String,
    server_side: String,
    project_type: String,
    downloads: u64,
    icon_url: Option<String>,
    project_id: String,
    author: String,
    versions: Vec<String>,
    follows: u64,
    date_created: String,
    date_modified: String,
    latest_version: String,
    license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModrinthProject {
    id: String,
    slug: String,
    title: String,
    description: String,
    body: String,
    categories: Vec<String>,
    client_side: String,
    server_side: String,
    downloads: u64,
    icon_url: Option<String>,
    team: String,
    body_url: Option<String>,
    published: String,
    updated: String,
    versions: Vec<String>,
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
    project_id: Option<String>,
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

pub struct ModManager {
    http_client: HttpClient,
    file_manager: FileManager,
    config: Config,
}

impl ModManager {
    pub fn new(config: Config, file_manager: FileManager) -> Self {
        // Create HTTP client with CurseForge API key
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static(CURSEFORGE_API_KEY));

        let http_client = HttpClient::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| HttpClient::new());

        Self {
            http_client,
            file_manager,
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

        // Execute search
        let response = self.http_client
            .get(url)
            .query(&query_params)
            .send()
            .await?
            .json::<CurseForgeSearchResponse>()
            .await?;

        // Convert CurseForge mods to our format
        let results = response.data.into_iter()
            .map(|cf_mod| self.convert_curseforge_mod(cf_mod))
            .collect();

        Ok(results)
    }

    fn convert_curseforge_mod(&self, cf_mod: CurseForgeMod) -> ModSearchResult {
        // Extract the latest file that matches our criteria
        let latest_file = cf_mod.latestFiles.first().cloned();

        // Extract versions
        let versions = cf_mod.latestFiles.iter()
            .map(|file| self.convert_curseforge_file(file))
            .collect();

        // Extract categories
        let categories = cf_mod.categories.iter()
            .map(|cat| cat.name.clone())
            .collect();

        // Extract author
        let author = cf_mod.authors.first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        // Extract icon URL
        let icon_url = cf_mod.logo.map(|logo| logo.url);

        // Create the result
        ModSearchResult {
            source: ModSource::CurseForge,
            id: cf_mod.id.to_string(),
            name: cf_mod.name,
            summary: cf_mod.summary,
            description: None, // Would need another API call to get full description
            author,
            download_count: cf_mod.downloadCount,
            follows: None, // CurseForge doesn't provide this
            icon_url,
            page_url: cf_mod.links.websiteUrl,
            versions,
            categories,
            latest_version: latest_file.as_ref().map(|f| f.displayName.clone()).unwrap_or_default(),
            latest_game_version: latest_file.as_ref()
                .and_then(|f| f.gameVersions.first().cloned())
                .unwrap_or_default(),
        }
    }

    fn convert_curseforge_file(&self, file: &CurseForgeFile) -> ModVersionInfo {
        // Determine mod loaders
        let mod_loaders = file.gameVersions.iter()
            .filter_map(|version| {
                if version.contains("Forge") {
                    Some(ModLoader::Forge)
                } else if version.contains("Fabric") {
                    Some(ModLoader::Fabric)
                } else if version.contains("Quilt") {
                    Some(ModLoader::Quilt)
                } else {
                    None
                }
            })
            .collect();

        // Extract game versions (excluding mod loader versions)
        let game_versions = file.gameVersions.iter()
            .filter(|v| !v.contains("Forge") && !v.contains("Fabric") && !v.contains("Quilt"))
            .cloned()
            .collect();

        // Extract dependencies
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

        // Extract SHA1 hash if available
        let sha1_hash = file.hashes.iter()
            .find(|hash| hash.algo == 1) // 1 = SHA1
            .map(|hash| hash.value.clone());

        ModVersionInfo {
            id: file.id.to_string(),
            name: file.displayName.clone(),
            version_number: file.displayName.clone(), // CurseForge doesn't have a separate version number
            game_versions,
            mod_loaders,
            download_url: file.downloadUrl.clone(),
            file_name: file.fileName.clone(),
            file_size: file.fileLength,
            release_date: file.fileDate.clone(),
            dependencies,
            sha1_hash,
        }
    }

    async fn search_modrinth(&self, params: &ModSearchParams) -> Result<Vec<ModSearchResult>> {
        // Build search URL
        let url = format!("{}/search", MODRINTH_API_BASE);

        // Convert ModLoader to Modrinth format
        let facets = self.build_modrinth_facets(params);

        // Build query parameters
        let mut query_params = vec![
            ("query", params.query.clone()),
            ("limit", params.limit.to_string()),
            ("offset", params.offset.to_string()),
            ("facets", facets),
        ];

        // Add sort field
        let sort_field = match params.sort_by {
            ModSortField::Featured => "featured",
            ModSortField::Popularity => "downloads",
            ModSortField::LastUpdated => "updated",
            ModSortField::Name => "relevance", // Modrinth doesn't have name sort
            ModSortField::Author => "relevance", // Modrinth doesn't have author sort
            ModSortField::TotalDownloads => "downloads",
            ModSortField::Category => "relevance", // Modrinth doesn't have category sort
        };
        query_params.push(("index", sort_field.to_string()));

        // Execute search
        let response = self.http_client
            .get(url)
            .query(&query_params)
            .send()
            .await?
            .json::<ModrinthSearchResponse>()
            .await?;

        // Convert search hits to full projects
        let mut results = Vec::new();
        for hit in response.hits {
            match self.get_modrinth_project(&hit.project_id).await {
                Ok(project) => {
                    if let Ok(versions) = self.get_modrinth_versions(&hit.project_id).await {
                        results.push(self.convert_modrinth_project(project, versions));
                    }
                },
                Err(e) => warn!("Error fetching Modrinth project {}: {}", hit.project_id, e),
            }
        }

        Ok(results)
    }

    fn build_modrinth_facets(&self, params: &ModSearchParams) -> String {
        let mut facets = Vec::new();

        // Add version facet
        if !params.minecraft_version.is_empty() {
            facets.push(format!("[\"versions:{}\"]", params.minecraft_version));
        }

        // Add category facet
        if let Some(category) = &params.category {
            facets.push(format!("[\"categories:{}\"]", category));
        }

        // Add mod loader facet
        if let Some(loader) = &params.mod_loader {
            let loader_str = match loader {
                ModLoader::Forge => "forge",
                ModLoader::Fabric => "fabric",
                ModLoader::Quilt => "quilt",
                ModLoader::None => return "".to_string(),
            };
            facets.push(format!("[\"categories:{}\"]", loader_str));
        }

        // Join facets
        if facets.is_empty() {
            "".to_string()
        } else {
            format!("[{}]", facets.join(","))
        }
    }

    async fn get_modrinth_project(&self, project_id: &str) -> Result<ModrinthProject> {
        let url = format!("{}/project/{}", MODRINTH_API_BASE, project_id);
        let project = self.http_client
            .get(url)
            .send()
            .await?
            .json::<ModrinthProject>()
            .await?;

        Ok(project)
    }

    async fn get_modrinth_versions(&self, project_id: &str) -> Result<Vec<ModrinthVersion>> {
        let url = format!("{}/project/{}/version", MODRINTH_API_BASE, project_id);
        let versions = self.http_client
            .get(url)
            .send()
            .await?
            .json::<Vec<ModrinthVersion>>()
            .await?;

        Ok(versions)
    }

    fn convert_modrinth_project(&self, project: ModrinthProject, versions: Vec<ModrinthVersion>) -> ModSearchResult {
        // Convert versions
        let version_infos = versions.iter()
            .map(|v| self.convert_modrinth_version(v))
            .collect::<Vec<_>>();

        // Get latest version info
        let latest_version = versions.first()
            .map(|v| v.version_number.clone())
            .unwrap_or_default();

        let latest_game_version = versions.first()
            .and_then(|v| v.game_versions.first().cloned())
            .unwrap_or_default();

        ModSearchResult {
            source: ModSource::Modrinth,
            id: project.id,
            name: project.title,
            summary: project.description,
            description: Some(project.body),
            author: project.team, // This is actually a team ID, would need another API call to get names
            download_count: project.downloads,
            follows: None, // Modrinth doesn't provide this in the project response
            icon_url: project.icon_url,
            page_url: format!("https://modrinth.com/mod/{}", project.slug),
            versions: version_infos,
            categories: project.categories,
            latest_version,
            latest_game_version,
        }
    }

    fn convert_modrinth_version(&self, version: &ModrinthVersion) -> ModVersionInfo {
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

        // Get a primary file
        let primary_file = version.files.iter()
            .find(|f| f.primary)
            .or_else(|| version.files.first())
            .cloned()
            .unwrap_or_else(|| ModrinthFile {
                hashes: HashMap::new(),
                url: String::new(),
                filename: String::new(),
                primary: true,
                size: 0,
            });

        // Extract SHA1 hash if available
        let sha1_hash = primary_file.hashes.get("sha1").cloned();

        // Convert dependencies
        let dependencies = version.dependencies.iter()
            .filter_map(|dep| {
                let project_id = match &dep.project_id {
                    Some(id) => id.clone(),
                    None => return None,
                };

                let dependency_type = match dep.dependency_type.as_str() {
                    "required" => DependencyType::Required,
                    "optional" => DependencyType::Optional,
                    "incompatible" => DependencyType::Incompatible,
                    "embedded" => DependencyType::Embedded,
                    _ => return None,
                };

                Some(ModDependency {
                    mod_id: project_id,
                    version_id: dep.version_id.clone(),
                    dependency_type,
                })
            })
            .collect();

        ModVersionInfo {
            id: version.id.clone(),
            name: version.name.clone(),
            version_number: version.version_number.clone(),
            game_versions: version.game_versions.clone(),
            mod_loaders,
            download_url: primary_file.url,
            file_name: primary_file.filename,
            file_size: primary_file.size,
            release_date: version.date_published.clone(),
            dependencies,
            sha1_hash,
        }
    }

    fn sort_results(&self, results: &mut Vec<ModSearchResult>, sort_by: &ModSortField, sort_order: &SortOrder) {
        results.sort_by(|a, b| {
            let cmp = match sort_by {
                ModSortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                ModSortField::Author => a.author.to_lowercase().cmp(&b.author.to_lowercase()),
                ModSortField::TotalDownloads => a.download_count.cmp(&b.download_count),
                ModSortField::LastUpdated => {
                    // This is a simplification, would need to parse dates properly
                    a.versions.first().map(|v| &v.release_date).cmp(&b.versions.first().map(|v| &v.release_date))
                },
                ModSortField::Category => {
                    a.categories.first().map(|c| c.to_lowercase()).cmp(&b.categories.first().map(|c| c.to_lowercase()))
                },
                // For other fields, fall back to download count
                _ => a.download_count.cmp(&b.download_count),
            };

            match sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });
    }

    pub async fn install_mod(&self, profile: &mut Profile, mod_result: &ModSearchResult, version: &ModVersionInfo, progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static) -> Result<()> {
        info!("Installing mod {} version {} for profile {}", mod_result.name, version.version_number, profile.name);

        // Determine mods directory
        let mods_dir = match &profile.game_directory {
            Some(dir) => PathBuf::from(dir).join("mods"),
            None => self.get_minecraft_directory().join("mods"),
        };

        // Create mods directory if it doesn't exist
        self.file_manager.create_dir_all(&mods_dir).await?;

        // Download the mod file
        let mod_path = mods_dir.join(&version.file_name);
        self.file_manager.download_file(
            &version.download_url,
            &mod_path,
            version.sha1_hash.as_deref(),
            progress_callback,
        ).await?;

        // Add the mod to the profile
        let mod_entry = Mod {
            id: mod_result.id.clone(),
            name: mod_result.name.clone(),
            version: version.version_number.clone(),
            source: mod_result.source.clone(),
            enabled: true,
        };

        // Remove any existing mod with the same ID
        profile.mods.retain(|m| m.id != mod_entry.id);

        // Add the new mod
        profile.mods.push(mod_entry);

        info!("Successfully installed mod {} for profile {}", mod_result.name, profile.name);
        Ok(())
    }

    pub async fn uninstall_mod(&self, profile: &mut Profile, mod_id: &str) -> Result<()> {
        info!("Uninstalling mod with ID {} from profile {}", mod_id, profile.name);

        // Find the mod in the profile
        let mod_entry = profile.mods.iter()
            .find(|m| m.id == mod_id)
            .ok_or_else(|| anyhow!("Mod with ID {} not found in profile {}", mod_id, profile.name))?;

        // Determine mods directory
        let mods_dir = match &profile.game_directory {
            Some(dir) => PathBuf::from(dir).join("mods"),
            None => self.get_minecraft_directory().join("mods"),
        };

        // Find and remove the mod file
        // This is a simplification - in a real implementation, we would need to store the filename
        let mut entry_stream = tokio::fs::read_dir(mods_dir).await?;
        while let Some(entry) = entry_stream.next_entry().await? {
            let file_name = entry.file_name().to_string_lossy().to_string();

            // This is a very simple heuristic - in a real implementation, we would need better tracking
            if file_name.contains(&mod_entry.name) || file_name.contains(&mod_entry.id) {
                fs::remove_file(entry.path()).await?;
                info!("Removed mod file: {:?}", entry.path());
            }
        }

        // Remove the mod from the profile
        profile.mods.retain(|m| m.id != mod_id);

        info!("Successfully uninstalled mod with ID {} from profile {}", mod_id, profile.name);
        Ok(())
    }

    pub async fn check_for_updates(&self, profile: &Profile) -> Result<HashMap<String, ModVersionInfo>> {
        info!("Checking for updates for profile {}", profile.name);

        let mut updates = HashMap::new();

        for mod_entry in &profile.mods {
            match mod_entry.source {
                ModSource::CurseForge => {
                    if let Ok(latest) = self.check_curseforge_update(mod_entry, &profile.version, &profile.mod_loader).await {
                        if latest.version_number != mod_entry.version {
                            updates.insert(mod_entry.id.clone(), latest);
                        }
                    }
                },
                ModSource::Modrinth => {
                    if let Ok(latest) = self.check_modrinth_update(mod_entry, &profile.version, &profile.mod_loader).await {
                        if latest.version_number != mod_entry.version {
                            updates.insert(mod_entry.id.clone(), latest);
                        }
                    }
                },
                ModSource::Manual => {
                    // Can't check for updates for manually installed mods
                }
            }
        }

        info!("Found {} updates for profile {}", updates.len(), profile.name);
        Ok(updates)
    }

    async fn check_curseforge_update(&self, mod_entry: &Mod, game_version: &str, mod_loader: &Option<ModLoader>) -> Result<ModVersionInfo> {
        let url = format!("{}/mods/{}/files", CURSEFORGE_API_BASE, mod_entry.id);

        // Build query parameters
        let mut query_params = vec![
            ("gameVersion", game_version.to_string()),
        ];

        // Add mod loader filter
        if let Some(loader) = mod_loader {
            let class_id = match loader {
                ModLoader::Forge => FORGE_MODLOADER_ID,
                ModLoader::Fabric => FABRIC_MODLOADER_ID,
                ModLoader::Quilt => QUILT_MODLOADER_ID,
                ModLoader::None => return Err(anyhow!("Cannot check for updates with no mod loader")),
            };
            query_params.push(("modLoaderType", class_id.to_string()));
        }

        // Execute query
        let response = self.http_client
            .get(url)
            .query(&query_params)
            .send()
            .await?
            .json::<CurseForgeResponse<Vec<CurseForgeFile>>>()
            .await?;

        // Get the latest file
        let latest_file = response.data.first()
            .ok_or_else(|| anyhow!("No files found for mod {}", mod_entry.id))?;

        Ok(self.convert_curseforge_file(latest_file))
    }

    async fn check_modrinth_update(&self, mod_entry: &Mod, game_version: &str, mod_loader: &Option<ModLoader>) -> Result<ModVersionInfo> {
        let url = format!("{}/project/{}/version", MODRINTH_API_BASE, mod_entry.id);

        // Build query parameters
        let mut query_params = vec![
            ("game_versions", format!("[\"{}\"]", game_version)),
        ];

        // Add mod loader filter
        if let Some(loader) = mod_loader {
            let loader_str = match loader {
                ModLoader::Forge => "forge",
                ModLoader::Fabric => "fabric",
                ModLoader::Quilt => "quilt",
                ModLoader::None => return Err(anyhow!("Cannot check for updates with no mod loader")),
            };
            query_params.push(("loaders", format!("[\"{}\"]", loader_str)));
        }

        // Execute query
        let versions = self.http_client
            .get(url)
            .query(&query_params)
            .send()
            .await?
            .json::<Vec<ModrinthVersion>>()
            .await?;

        // Get the latest version
        let latest_version = versions.first()
            .ok_or_else(|| anyhow!("No versions found for mod {}", mod_entry.id))?;

        Ok(self.convert_modrinth_version(latest_version))
    }
}
