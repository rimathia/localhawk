//! Image cache implementation using the generic LRU framework

use super::{CacheConfig, FileStorage, LruCache};
use crate::error::ProxyError;
use std::path::PathBuf;

const MAGIC_CARD_SIZE_ESTIMATE: u64 = 956 * 1024; // 480x680 pixels * 3 bytes ≈ 956 KB
const DEFAULT_MAX_SIZE_MB: u64 = 1000;

/// Image cache type alias
pub type LruImageCache = LruCache<String, Vec<u8>, FileStorage>;

/// Create a new image cache with sensible defaults for Magic card images
pub fn create_image_cache() -> Result<LruImageCache, ProxyError> {
    create_image_cache_with_config(None, DEFAULT_MAX_SIZE_MB * 1024 * 1024)
}

/// Create a new image cache with custom configuration
pub fn create_image_cache_with_config(
    cache_dir: Option<PathBuf>,
    max_size_bytes: u64,
) -> Result<LruImageCache, ProxyError> {
    let cache_dir = cache_dir.unwrap_or_else(|| PathBuf::from(crate::get_cache_directory_path()));

    let storage = FileStorage::new(
        cache_dir,
        "jpg".to_string(), // Default extension for card images
        MAGIC_CARD_SIZE_ESTIMATE,
    )?;

    let config = CacheConfig {
        max_entries: None, // No entry limit, only size limit
        max_size_bytes: Some(max_size_bytes),
        eager_persistence: false, // Save only on shutdown for performance
    };

    LruCache::new(storage, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_image_cache_basic() {
        let temp_dir = env::temp_dir().join(format!("localhawk-lru-test-{}", std::process::id()));
        let mut cache =
            create_image_cache_with_config(Some(temp_dir.clone()), 1024 * 1024).unwrap();

        // Test basic operations
        assert!(cache.is_empty());

        let test_image = vec![1, 2, 3, 4, 5];
        let url = "https://example.com/test.jpg".to_string();

        cache.insert(url.clone(), test_image.clone()).unwrap();
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get(&url);
        assert_eq!(retrieved, Some(test_image));

        // Clean up
        cache.clear().unwrap();
        if temp_dir.exists() {
            std::fs::remove_dir_all(temp_dir).ok();
        }
    }

    #[test]
    fn test_image_cache_size_limit() {
        let temp_dir = env::temp_dir().join(format!("localhawk-size-test-{}", std::process::id()));
        let mut cache = create_image_cache_with_config(Some(temp_dir.clone()), 100).unwrap(); // Very small limit

        // Add an image that's larger than the cache limit
        let large_image = vec![0u8; 200]; // 200 bytes, larger than 100 byte limit
        let url = "https://example.com/large.jpg".to_string();

        // This should work, evicting as needed
        cache.insert(url.clone(), large_image.clone()).unwrap();

        let retrieved = cache.get(&url);
        assert_eq!(retrieved, Some(large_image));

        // Clean up
        cache.clear().unwrap();
        if temp_dir.exists() {
            std::fs::remove_dir_all(temp_dir).ok();
        }
    }
}
