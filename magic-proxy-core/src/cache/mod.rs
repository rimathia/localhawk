use printpdf::image_crate::DynamicImage;
use std::collections::HashMap;
use time::{Duration, OffsetDateTime};

#[derive(Debug)]
struct CacheEntry {
    image: DynamicImage,
    created_at: OffsetDateTime,
}

#[derive(Debug)]
pub struct ImageCache {
    cache: HashMap<String, CacheEntry>,
    max_age: Duration,
}

impl ImageCache {
    pub fn new() -> Self {
        ImageCache {
            cache: HashMap::new(),
            max_age: Duration::days(14), // Cache for 14 days like MagicHawk
        }
    }

    pub fn with_max_age(max_age: Duration) -> Self {
        ImageCache {
            cache: HashMap::new(),
            max_age,
        }
    }

    pub fn get(&self, url: &str) -> Option<&DynamicImage> {
        self.cache.get(url).and_then(|entry| {
            let age = OffsetDateTime::now_utc() - entry.created_at;
            if age < self.max_age {
                Some(&entry.image)
            } else {
                None
            }
        })
    }

    pub fn insert(&mut self, url: String, image: DynamicImage) {
        self.cache.insert(
            url,
            CacheEntry {
                image,
                created_at: OffsetDateTime::now_utc(),
            },
        );
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn purge_expired(&mut self) {
        let now = OffsetDateTime::now_utc();
        self.cache.retain(|_, entry| {
            let age = now - entry.created_at;
            age < self.max_age
        });
    }

    pub fn size(&self) -> usize {
        self.cache.len()
    }

    pub fn contains(&self, url: &str) -> bool {
        if let Some(entry) = self.cache.get(url) {
            let age = OffsetDateTime::now_utc() - entry.created_at;
            age < self.max_age
        } else {
            false
        }
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_cache_basic_operations() {
        let mut cache = ImageCache::new();
        let test_url = "http://example.com/test.jpg";
        let test_image = create_test_image();

        // Test insertion and retrieval
        cache.insert(test_url.to_string(), test_image.clone());
        assert_eq!(cache.size(), 1);
        assert!(cache.contains(test_url));

        let retrieved = cache.get(test_url);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_cache_expiration() {
        let mut cache = ImageCache::with_max_age(Duration::seconds(1));
        let test_url = "http://example.com/test.jpg";
        let test_image = create_test_image();

        cache.insert(test_url.to_string(), test_image);
        assert!(cache.contains(test_url));

        // Simulate time passing by creating new cache with very short duration
        // This is a simplified test - in real usage you'd wait for actual time to pass
        let mut expired_cache = ImageCache::with_max_age(Duration::seconds(-1));
        expired_cache.insert(test_url.to_string(), create_test_image());
        assert!(!expired_cache.contains(test_url));
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = ImageCache::new();
        cache.insert("url1".to_string(), create_test_image());
        cache.insert("url2".to_string(), create_test_image());

        assert_eq!(cache.size(), 2);
        cache.clear();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_cache_purge_expired() {
        let mut cache = ImageCache::new();
        cache.insert("url1".to_string(), create_test_image());

        assert_eq!(cache.size(), 1);
        cache.purge_expired(); // Should not remove anything since max_age is 14 days
        assert_eq!(cache.size(), 1);
    }
}
