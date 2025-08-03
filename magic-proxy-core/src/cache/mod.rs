use printpdf::image_crate::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use time::OffsetDateTime;
use tracing::{debug, info, warn};
use crate::error::ProxyError;

#[derive(Debug)]
struct CacheEntry {
    image: DynamicImage,
    created_at: OffsetDateTime,
    last_accessed: OffsetDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskCacheMetadata {
    entries: HashMap<String, DiskCacheEntry>,
    total_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct DiskCacheEntry {
    url: String,
    filename: String,
    created_at: OffsetDateTime,
    last_accessed: OffsetDateTime,
    size_bytes: u64,
}

#[derive(Debug)]
pub struct ImageCache {
    cache: HashMap<String, CacheEntry>,
    cache_dir: PathBuf,
    metadata_file: PathBuf,
    max_size_bytes: u64,
    image_size_estimate: u64,
}

const MAGIC_CARD_SIZE_ESTIMATE: u64 = 956 * 1024; // 480x680 pixels * 3 bytes ≈ 956 KB
const DEFAULT_MAX_SIZE_MB: u64 = 100;
const METADATA_FILENAME: &str = "cache_metadata.json";

impl ImageCache {
    pub fn new() -> Result<Self, ProxyError> {
        Self::with_cache_dir_and_size(None, DEFAULT_MAX_SIZE_MB * 1024 * 1024)
    }

    pub fn with_cache_dir_and_size(cache_dir: Option<PathBuf>, max_size_bytes: u64) -> Result<Self, ProxyError> {
        let cache_dir = cache_dir.unwrap_or_else(|| {
            directories::ProjectDirs::from("", "", "magic-proxy")
                .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
                .unwrap_or_else(|| std::env::temp_dir().join("magic-proxy-cache"))
        });
        
        let metadata_file = cache_dir.join(METADATA_FILENAME);
        
        // Create cache directory if it doesn't exist
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)
                .map_err(|e| ProxyError::Io(e))?;
            info!(cache_dir = %cache_dir.display(), "Created image cache directory");
        }
        
        let mut cache = ImageCache {
            cache: HashMap::new(),
            cache_dir,
            metadata_file,
            max_size_bytes,
            image_size_estimate: MAGIC_CARD_SIZE_ESTIMATE,
        };
        
        cache.load_from_disk()?;
        Ok(cache)
    }

    pub fn get(&mut self, url: &str) -> Option<&DynamicImage> {
        if let Some(entry) = self.cache.get_mut(url) {
            entry.last_accessed = OffsetDateTime::now_utc();
            debug!(url = %url, "Image cache HIT");
            Some(&entry.image)
        } else {
            debug!(url = %url, "Image cache MISS");
            None
        }
    }

    pub fn insert(&mut self, url: String, image: DynamicImage) -> Result<(), ProxyError> {
        let now = OffsetDateTime::now_utc();
        
        // Check if we need to evict entries to make room
        self.ensure_space_for_new_entry()?;
        
        // Generate filename from URL hash
        let filename = self.url_to_filename(&url);
        let file_path = self.cache_dir.join(&filename);
        
        // Save image to disk
        image.save_with_format(&file_path, ImageFormat::Jpeg)
            .map_err(|e| ProxyError::Cache(format!("Failed to save image to disk: {}", e)))?;
        
        // Insert into memory cache
        self.cache.insert(
            url.clone(),
            CacheEntry {
                image,
                created_at: now,
                last_accessed: now,
            },
        );
        
        // Update disk metadata
        self.save_metadata_to_disk()?;
        
        debug!(url = %url, filename = %filename, cache_dir = %self.cache_dir.display(), "Image cached to disk");
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), ProxyError> {
        // Remove all files from disk
        for entry in self.cache.keys() {
            let filename = self.url_to_filename(entry);
            let file_path = self.cache_dir.join(&filename);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    warn!(file = %file_path.display(), error = %e, "Failed to remove cached image file");
                }
            }
        }
        
        self.cache.clear();
        self.save_metadata_to_disk()?;
        info!("Cleared all cached images");
        Ok(())
    }
    
    pub fn force_evict(&mut self, url: &str) -> Result<(), ProxyError> {
        if let Some(_) = self.cache.remove(url) {
            let filename = self.url_to_filename(url);
            let file_path = self.cache_dir.join(&filename);
            if file_path.exists() {
                fs::remove_file(&file_path)
                    .map_err(|e| ProxyError::Io(e))?;
            }
            self.save_metadata_to_disk()?;
            debug!(url = %url, "Force evicted image from cache");
        }
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }
    
    pub fn size_bytes(&self) -> u64 {
        (self.cache.len() as u64) * self.image_size_estimate
    }

    pub fn contains(&self, url: &str) -> bool {
        self.cache.contains_key(url)
    }
    
    fn url_to_filename(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let hash = hasher.finalize();
        format!("{:x}.jpg", hash)
    }
    
    fn ensure_space_for_new_entry(&mut self) -> Result<(), ProxyError> {
        let current_size = self.size_bytes();
        let new_entry_size = self.image_size_estimate;
        
        if current_size + new_entry_size <= self.max_size_bytes {
            return Ok(());
        }
        
        // Need to evict entries using LRU
        let mut entries_by_access: Vec<_> = self.cache.iter()
            .map(|(url, entry)| (url.clone(), entry.last_accessed))
            .collect();
        
        // Sort by last_accessed (oldest first)
        entries_by_access.sort_by_key(|(_, last_accessed)| *last_accessed);
        
        let mut size_freed = 0u64;
        let mut urls_to_remove = Vec::new();
        
        for (url, _) in entries_by_access {
            if current_size - size_freed + new_entry_size <= self.max_size_bytes {
                break;
            }
            urls_to_remove.push(url);
            size_freed += self.image_size_estimate;
        }
        
        info!(entries_to_evict = urls_to_remove.len(), size_freed_kb = size_freed / 1024, "Evicting LRU cache entries");
        
        for url in urls_to_remove {
            self.force_evict(&url)?;
        }
        
        Ok(())
    }
    
    fn load_from_disk(&mut self) -> Result<(), ProxyError> {
        if !self.metadata_file.exists() {
            debug!(metadata_file = %self.metadata_file.display(), cache_dir = %self.cache_dir.display(), "No existing cache metadata found");
            return Ok(());
        }
        
        let metadata_content = fs::read_to_string(&self.metadata_file)
            .map_err(|e| ProxyError::Io(e))?;
        
        let metadata: DiskCacheMetadata = serde_json::from_str(&metadata_content)
            .map_err(|e| ProxyError::Json(e))?;
        
        let mut loaded_count = 0;
        let mut failed_count = 0;
        
        for (url, disk_entry) in metadata.entries {
            let file_path = self.cache_dir.join(&disk_entry.filename);
            
            if !file_path.exists() {
                debug!(url = %url, file = %file_path.display(), "Cached image file missing, skipping");
                failed_count += 1;
                continue;
            }
            
            match printpdf::image_crate::open(&file_path) {
                Ok(image) => {
                    self.cache.insert(url, CacheEntry {
                        image,
                        created_at: disk_entry.created_at,
                        last_accessed: disk_entry.last_accessed,
                    });
                    loaded_count += 1;
                }
                Err(e) => {
                    warn!(url = %url, file = %file_path.display(), error = %e, "Failed to load cached image");
                    failed_count += 1;
                }
            }
        }
        
        info!(
            loaded = loaded_count,
            failed = failed_count, 
            cache_size_mb = self.size_bytes() / (1024 * 1024),
            cache_dir = %self.cache_dir.display(),
            "Loaded image cache from disk"
        );
        
        Ok(())
    }
    
    fn save_metadata_to_disk(&self) -> Result<(), ProxyError> {
        let mut entries = HashMap::new();
        let mut total_size = 0u64;
        
        for (url, entry) in &self.cache {
            let filename = self.url_to_filename(url);
            entries.insert(url.clone(), DiskCacheEntry {
                url: url.clone(),
                filename,
                created_at: entry.created_at,
                last_accessed: entry.last_accessed,
                size_bytes: self.image_size_estimate,
            });
            total_size += self.image_size_estimate;
        }
        
        let metadata = DiskCacheMetadata {
            entries,
            total_size_bytes: total_size,
        };
        
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| ProxyError::Json(e))?;
        
        fs::write(&self.metadata_file, metadata_json)
            .map_err(|e| ProxyError::Io(e))?;
        
        debug!(metadata_file = %self.metadata_file.display(), entries = self.cache.len(), cache_dir = %self.cache_dir.display(), "Saved cache metadata to disk");
        Ok(())
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new().expect("Failed to create default image cache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use printpdf::image_crate::{DynamicImage, RgbImage};

    fn create_test_image() -> DynamicImage {
        let img = RgbImage::new(100, 100);
        DynamicImage::ImageRgb8(img)
    }
    
    fn create_test_cache() -> Result<ImageCache, ProxyError> {
        let temp_dir = std::env::temp_dir().join(format!("magic-proxy-test-{}", std::process::id()));
        ImageCache::with_cache_dir_and_size(Some(temp_dir), 10 * 1024 * 1024)
    }

    #[test]
    fn test_cache_basic_operations() {
        let mut cache = create_test_cache().unwrap();
        let test_url = "http://example.com/test.jpg";
        let test_image = create_test_image();

        // Test insertion and retrieval
        cache.insert(test_url.to_string(), test_image.clone()).unwrap();
        assert_eq!(cache.size(), 1);
        assert!(cache.contains(test_url));

        let retrieved = cache.get(test_url);
        assert!(retrieved.is_some());
        
        // Clean up
        cache.clear().unwrap();
    }

    #[test]
    fn test_cache_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("magic-proxy-test-persist-{}", std::process::id()));
        let test_url = "http://example.com/test.jpg";
        let test_image = create_test_image();
        
        // Create cache and add image
        {
            let mut cache = ImageCache::with_cache_dir_and_size(Some(temp_dir.clone()), 10 * 1024 * 1024).unwrap();
            cache.insert(test_url.to_string(), test_image).unwrap();
            assert_eq!(cache.size(), 1);
        }
        
        // Create new cache with same directory - should load from disk
        {
            let mut cache = ImageCache::with_cache_dir_and_size(Some(temp_dir.clone()), 10 * 1024 * 1024).unwrap();
            assert_eq!(cache.size(), 1);
            assert!(cache.contains(test_url));
            
            // Clean up
            cache.clear().unwrap();
        }
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = create_test_cache().unwrap();
        cache.insert("url1".to_string(), create_test_image()).unwrap();
        cache.insert("url2".to_string(), create_test_image()).unwrap();

        assert_eq!(cache.size(), 2);
        cache.clear().unwrap();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_cache_lru_eviction() {
        // Create cache with very small size to force eviction
        let temp_dir = std::env::temp_dir().join(format!("magic-proxy-test-lru-{}", std::process::id()));
        let mut cache = ImageCache::with_cache_dir_and_size(Some(temp_dir), 2 * MAGIC_CARD_SIZE_ESTIMATE).unwrap();
        
        // Fill cache to capacity
        cache.insert("url1".to_string(), create_test_image()).unwrap();
        cache.insert("url2".to_string(), create_test_image()).unwrap();
        assert_eq!(cache.size(), 2);
        
        // Access first image to make it more recently used
        let _ = cache.get("url1");
        
        // Insert third image - should evict url2 (least recently used)
        cache.insert("url3".to_string(), create_test_image()).unwrap();
        assert_eq!(cache.size(), 2);
        assert!(cache.contains("url1")); // Should still be there
        assert!(!cache.contains("url2")); // Should be evicted
        assert!(cache.contains("url3")); // Should be there
        
        // Clean up
        cache.clear().unwrap();
    }
    
    #[test]
    fn test_force_evict() {
        let mut cache = create_test_cache().unwrap();
        let test_url = "http://example.com/test.jpg";
        
        cache.insert(test_url.to_string(), create_test_image()).unwrap();
        assert!(cache.contains(test_url));
        
        cache.force_evict(test_url).unwrap();
        assert!(!cache.contains(test_url));
        assert_eq!(cache.size(), 0);
    }
    
    #[test]
    fn test_url_hashing() {
        let cache = create_test_cache().unwrap();
        let url1 = "https://cards.scryfall.io/border_crop/front/7/7/77c6fa74-5543-42ac-9ead-0e890b188e99.jpg?1706239968";
        let url2 = "https://cards.scryfall.io/border_crop/front/8/8/88888888-5543-42ac-9ead-0e890b188e99.jpg?1706239968";
        
        let filename1 = cache.url_to_filename(url1);
        let filename2 = cache.url_to_filename(url2);
        
        // Different URLs should produce different filenames
        assert_ne!(filename1, filename2);
        
        // Same URL should produce same filename
        assert_eq!(filename1, cache.url_to_filename(url1));
        
        // Should be valid .jpg files
        assert!(filename1.ends_with(".jpg"));
        assert!(filename2.ends_with(".jpg"));
    }
}
