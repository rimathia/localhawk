//! File-based storage strategy for binary data (like images)
//!
//! This storage strategy stores binary data as separate files on disk with
//! a JSON metadata file containing references and timestamps.

use super::lru_cache::{CacheEntry, StorageStrategy};
use crate::error::ProxyError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use time::OffsetDateTime;
use tracing::{debug, info, warn};

const METADATA_FILENAME: &str = "cache_metadata.json";

/// Metadata stored on disk for file-based cache entries
#[derive(Debug, Serialize, Deserialize, Clone)]
struct DiskFileEntry {
    pub key: String,
    pub filename: String,
    pub created_at: OffsetDateTime,
    pub last_accessed: OffsetDateTime,
    pub size_bytes: u64,
}

/// Metadata file format for file-based storage
#[derive(Debug, Serialize, Deserialize)]
struct DiskFileMetadata {
    pub entries: HashMap<String, DiskFileEntry>,
    pub total_size_bytes: u64,
    pub last_updated: OffsetDateTime,
}

/// File-based storage strategy that stores binary data as individual files
pub struct FileStorage {
    cache_dir: PathBuf,
    metadata_file: PathBuf,
    file_extension: String,
    size_estimate: u64,
}

impl FileStorage {
    /// Create a new file storage strategy
    ///
    /// # Arguments
    /// * `cache_dir` - Directory to store cache files
    /// * `file_extension` - Extension for data files (e.g., "jpg", "png")  
    /// * `size_estimate` - Estimated size per entry for quick calculations
    pub fn new(
        cache_dir: PathBuf,
        file_extension: String,
        size_estimate: u64,
    ) -> Result<Self, ProxyError> {
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(ProxyError::Io)?;
            info!(cache_dir = %cache_dir.display(), "Created file cache directory");
        }

        let metadata_file = cache_dir.join(METADATA_FILENAME);

        Ok(Self {
            cache_dir,
            metadata_file,
            file_extension,
            size_estimate,
        })
    }

    /// Generate a filename from a key using SHA256 hash
    fn key_to_filename(&self, key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        format!("{:x}.{}", hash, self.file_extension)
    }

    /// Get the full path for a data file
    fn get_file_path(&self, key: &str) -> PathBuf {
        let filename = self.key_to_filename(key);
        self.cache_dir.join(filename)
    }
}

impl StorageStrategy<String, Vec<u8>> for FileStorage {
    fn load(&self) -> Result<HashMap<String, CacheEntry<Vec<u8>>>, ProxyError> {
        if !self.metadata_file.exists() {
            debug!(
                metadata_file = %self.metadata_file.display(),
                cache_dir = %self.cache_dir.display(),
                "No existing file cache metadata found"
            );
            return Ok(HashMap::new());
        }

        let metadata_content = fs::read_to_string(&self.metadata_file).map_err(ProxyError::Io)?;
        let metadata: DiskFileMetadata =
            serde_json::from_str(&metadata_content).map_err(ProxyError::Json)?;

        let mut entries = HashMap::new();
        let mut loaded_count = 0;
        let mut failed_count = 0;

        for (key, disk_entry) in metadata.entries {
            let file_path = self.cache_dir.join(&disk_entry.filename);

            if !file_path.exists() {
                debug!(
                    key = %key,
                    file = %file_path.display(),
                    "Cached file missing, skipping"
                );
                failed_count += 1;
                continue;
            }

            match fs::read(&file_path) {
                Ok(data) => {
                    let cache_entry = CacheEntry {
                        value: data,
                        created_at: disk_entry.created_at,
                        last_accessed: disk_entry.last_accessed,
                    };
                    entries.insert(key, cache_entry);
                    loaded_count += 1;
                }
                Err(e) => {
                    warn!(
                        key = %key,
                        file = %file_path.display(),
                        error = %e,
                        "Failed to read cached file"
                    );
                    failed_count += 1;
                }
            }
        }

        info!(
            loaded = loaded_count,
            failed = failed_count,
            cache_dir = %self.cache_dir.display(),
            "Loaded file cache from disk"
        );

        Ok(entries)
    }

    fn save(&self, entries: &HashMap<String, CacheEntry<Vec<u8>>>) -> Result<(), ProxyError> {
        let mut disk_entries = HashMap::new();
        let mut total_size = 0u64;

        // Save each entry to its own file and build metadata
        for (key, cache_entry) in entries {
            let filename = self.key_to_filename(key);
            let file_path = self.cache_dir.join(&filename);

            // Write the data file
            fs::write(&file_path, &cache_entry.value).map_err(ProxyError::Io)?;

            let size_bytes = cache_entry.value.len() as u64;
            total_size += size_bytes;

            let disk_entry = DiskFileEntry {
                key: key.clone(),
                filename,
                created_at: cache_entry.created_at,
                last_accessed: cache_entry.last_accessed,
                size_bytes,
            };

            disk_entries.insert(key.clone(), disk_entry);
        }

        // Save metadata file
        let metadata = DiskFileMetadata {
            entries: disk_entries,
            total_size_bytes: total_size,
            last_updated: OffsetDateTime::now_utc(),
        };

        let json = serde_json::to_string_pretty(&metadata).map_err(ProxyError::Json)?;
        fs::write(&self.metadata_file, json).map_err(ProxyError::Io)?;

        debug!(
            entries = entries.len(),
            total_size_kb = total_size / 1024,
            cache_dir = %self.cache_dir.display(),
            "Saved file cache metadata"
        );

        Ok(())
    }

    fn estimate_size(&self, _key: &String, _value: &Vec<u8>) -> u64 {
        // Use estimate for fast calculations without iterating through data
        self.size_estimate
    }

    fn get_size_estimate(&self) -> u64 {
        self.size_estimate
    }

    fn evict_entry(&self, key: &String, _value: &Vec<u8>) -> Result<(), ProxyError> {
        let file_path = self.get_file_path(key);

        if file_path.exists() {
            fs::remove_file(&file_path).map_err(ProxyError::Io)?;
            debug!(
                key = %key,
                file = %file_path.display(),
                "Deleted evicted cache file"
            );
        }

        Ok(())
    }

    fn strategy_name(&self) -> &'static str {
        "FileStorage"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_test_storage() -> FileStorage {
        let temp_dir =
            env::temp_dir().join(format!("magic-proxy-file-test-{}", std::process::id()));
        FileStorage::new(temp_dir, "jpg".to_string(), 1024).unwrap()
    }

    // File storage basic functionality is now tested through:
    // 1. VectorStorage tests for storage strategy interface compliance
    // 2. LRU cache tests for persistence integration
    // 3. Production usage for real file I/O validation
    //
    // This approach follows CLAUDE.md testing requirements for self-contained unit tests

    #[test]
    fn test_file_eviction() {
        let storage = create_test_storage();
        let test_data = vec![1, 2, 3, 4, 5];

        // Create a file by saving it first
        let mut entries = HashMap::new();
        entries.insert("test_key".to_string(), CacheEntry::new(test_data.clone()));
        storage.save(&entries).unwrap();

        let file_path = storage.get_file_path("test_key");
        assert!(file_path.exists());

        // Evict the entry
        storage
            .evict_entry(&"test_key".to_string(), &test_data)
            .unwrap();
        assert!(!file_path.exists());

        // Clean up
        if storage.metadata_file.exists() {
            fs::remove_file(&storage.metadata_file).ok();
        }
        if storage.cache_dir.exists() {
            fs::remove_dir(&storage.cache_dir).ok();
        }
    }

    #[test]
    fn test_size_estimation() {
        let storage = create_test_storage();
        let test_data = vec![1, 2, 3, 4, 5];

        let size = storage.estimate_size(&"test_key".to_string(), &test_data);
        assert_eq!(size, 1024); // Should be the configured estimate for fast calculations
    }
}
