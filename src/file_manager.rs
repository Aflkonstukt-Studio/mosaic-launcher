use anyhow::{Result, anyhow};
use reqwest::Client as HttpClient;
use sha1::{Sha1, Digest};
use sha2::{Sha256, Digest as Sha2Digest};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use futures::StreamExt;
use zip::ZipArchive;
use std::io::Cursor;
use std::fs::File;
use std::io::Read;
use log::{info, warn, error, debug};

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub url: String,
    pub file_name: String,
    pub total_size: Option<u64>,
    pub downloaded_size: u64,
    pub percentage: f32,
}

#[derive(Clone)]
pub struct FileManager {
    http_client: HttpClient,
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
        }
    }

    pub async fn create_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::create_dir_all(path).await?;
        Ok(())
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        path: P,
        expected_hash: Option<&str>,
        progress_callback: impl Fn(DownloadProgress) + Send + Sync + 'static,
    ) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent).await?;
        }

        // Check if file already exists and has the correct hash
        if path.exists() {
            if let Some(hash) = expected_hash {
                if self.verify_file_hash(path, hash).await? {
                    debug!("File already exists with correct hash: {:?}", path);
                    return Ok(());
                }
            }
        }

        // Start the download
        info!("Downloading {} to {:?}", url, path);

        let response = self.http_client.get(url).send().await?;
        let total_size = response.content_length();

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Create the file
        let mut file = fs::File::create(path).await?;

        // Download the file in chunks and report progress
        let mut stream = response.bytes_stream();
        let mut downloaded_size: u64 = 0;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;

            downloaded_size += chunk.len() as u64;
            let percentage = match total_size {
                Some(size) => (downloaded_size as f32 / size as f32) * 100.0,
                None => 0.0,
            };

            // Report progress
            progress_callback(DownloadProgress {
                url: url.to_string(),
                file_name: file_name.clone(),
                total_size,
                downloaded_size,
                percentage,
            });
        }

        // Verify hash if provided
        if let Some(hash) = expected_hash {
            if !self.verify_file_hash(path, hash).await? {
                fs::remove_file(path).await?;
                return Err(anyhow!("Hash verification failed for file: {:?}", path));
            }
        }

        info!("Download completed: {:?}", path);
        Ok(())
    }

    pub async fn verify_file_hash<P: AsRef<Path>>(&self, path: P, expected_hash: &str) -> Result<bool> {
        let path = path.as_ref();

        // Read the file
        let mut file = fs::File::open(path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        // Determine hash type by length
        let computed_hash = match expected_hash.len() {
            40 => {
                // SHA-1
                let mut hasher = Sha1::new();
                hasher.update(&buffer);
                format!("{:x}", hasher.finalize())
            },
            64 => {
                // SHA-256
                let mut hasher = Sha256::new();
                hasher.update(&buffer);
                format!("{:x}", hasher.finalize())
            },
            _ => {
                return Err(anyhow!("Unsupported hash length: {}", expected_hash.len()));
            }
        };

        Ok(computed_hash.eq_ignore_ascii_case(expected_hash))
    }

    pub async fn extract_zip<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        zip_path: P,
        extract_to: Q,
        exclude: Option<&Vec<String>>,
    ) -> Result<()> {
        let zip_path = zip_path.as_ref();
        let extract_to = extract_to.as_ref();

        info!("Extracting zip: {} -> {}", zip_path.display(), extract_to.display());
        if let Some(excludes) = exclude {
            info!("Exclusion patterns: {:?}", excludes);
        }

        // Create extraction directory if it doesn't exist
        self.create_dir_all(extract_to).await?;

        // Read the zip file
        let zip_data = fs::read(zip_path).await
            .map_err(|e| anyhow!("Failed to read zip file {}: {}", zip_path.display(), e))?;

        info!("Zip file size: {} bytes", zip_data.len());

        // Use a separate thread for zip extraction since it's CPU-bound
        let extract_to_owned = extract_to.to_path_buf();
        let exclude_owned = exclude.map(|e| e.to_vec());

        let result = tokio::task::spawn_blocking(move || -> Result<usize> {
            let cursor = Cursor::new(zip_data);
            let mut archive = ZipArchive::new(cursor)
                .map_err(|e| anyhow!("Failed to open zip archive: {}", e))?;

            let mut extracted_files = 0;
            let total_files = archive.len();

            info!("Zip archive contains {} entries", total_files);

            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .map_err(|e| anyhow!("Failed to read zip entry {}: {}", i, e))?;

                let file_path = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => {
                        warn!("Skipping entry {} with invalid name", i);
                        continue;
                    }
                };

                let file_path_str = file_path.to_string_lossy();
                debug!("Processing zip entry: {}", file_path_str);

                // Check if this file should be excluded
                let mut should_exclude = false;
                if let Some(exclude_list) = &exclude_owned {
                    for pattern in exclude_list {
                        // Use starts_with for directory patterns (like "META-INF/")
                        // and exact match or contains for file patterns
                        if pattern.ends_with('/') {
                            // Directory pattern - check if path starts with it
                            if file_path_str.starts_with(pattern) {
                                should_exclude = true;
                                debug!("Excluding {} (matches directory pattern {})", file_path_str, pattern);
                                break;
                            }
                        } else {
                            // File pattern - check if path contains it
                            if file_path_str.contains(pattern) {
                                should_exclude = true;
                                debug!("Excluding {} (matches file pattern {})", file_path_str, pattern);
                                break;
                            }
                        }
                    }
                }

                if should_exclude {
                    continue;
                }

                let target_path = extract_to_owned.join(&file_path);

                if file.is_dir() {
                    debug!("Creating directory: {}", target_path.display());
                    std::fs::create_dir_all(&target_path)
                        .map_err(|e| anyhow!("Failed to create directory {}: {}", target_path.display(), e))?;
                } else {
                    if let Some(parent) = target_path.parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)
                                .map_err(|e| anyhow!("Failed to create parent directory {}: {}", parent.display(), e))?;
                        }
                    }

                    debug!("Extracting file: {} ({} bytes)", target_path.display(), file.size());
                    let mut target_file = File::create(&target_path)
                        .map_err(|e| anyhow!("Failed to create file {}: {}", target_path.display(), e))?;

                    std::io::copy(&mut file, &mut target_file)
                        .map_err(|e| anyhow!("Failed to extract file {}: {}", target_path.display(), e))?;

                    extracted_files += 1;
                }
            }

            info!("Extracted {} files from {} total entries", extracted_files, total_files);
            Ok(extracted_files)
        }).await??;

        if result == 0 {
            warn!("No files were extracted from {}", zip_path.display());
        }

        info!("Successfully extracted {} files from {:?} to {:?}", result, zip_path, extract_to);
        Ok(())
    }

    pub async fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(&self, from: P, to: Q) -> Result<()> {
        let from = from.as_ref();
        let to = to.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = to.parent() {
            self.create_dir_all(parent).await?;
        }

        fs::copy(from, to).await?;
        Ok(())
    }

    pub async fn remove_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::remove_file(path).await?;
        Ok(())
    }

    pub async fn remove_dir_all<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::remove_dir_all(path).await?;
        Ok(())
    }

    pub async fn file_exists<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref().exists()
    }

    pub async fn read_to_string<P: AsRef<Path>>(&self, path: P) -> Result<String> {
        let content = fs::read_to_string(path).await?;
        Ok(content)
    }

    pub async fn write_to_file<P: AsRef<Path>, C: AsRef<[u8]>>(&self, path: P, contents: C) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent).await?;
        }

        fs::write(path, contents).await?;
        Ok(())
    }
}
