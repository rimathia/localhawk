use crate::{
    CardNameCache, CardNameLookup, NameLookupResult, ProxyError, ScryfallClient,
    SetCodesCache,
};
use crate::cache::{LruImageCache, create_image_cache, LruSearchCache, create_search_cache};
use directories::ProjectDirs;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{debug, info};

// Memory size estimation constants for cache statistics

// Card Names Cache: per-name estimate (avg 20 bytes) + fuzzy index overhead factor (4x for trie structure)
const CARD_NAME_SIZE_ESTIMATE: u64 = 20; // 20 bytes per card name
const FUZZY_INDEX_OVERHEAD_FACTOR: u64 = 4; // Fuzzy index adds 4x overhead for trie structure

// Global singletons - initialized once, shared everywhere
static SCRYFALL_CLIENT: OnceLock<ScryfallClient> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<LruImageCache>>> = OnceLock::new();
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<Arc<RwLock<LruSearchCache>>> = OnceLock::new();
static SET_CODES_CACHE: OnceLock<Arc<RwLock<Option<HashSet<String>>>>> = OnceLock::new();
static CARD_NAME_CACHE_INFO: OnceLock<Arc<RwLock<Option<(time::OffsetDateTime, usize)>>>> =
    OnceLock::new();

pub fn get_scryfall_client() -> &'static ScryfallClient {
    SCRYFALL_CLIENT.get_or_init(|| ScryfallClient::new().expect("Failed to create ScryfallClient"))
}

pub fn get_image_cache() -> &'static Arc<RwLock<LruImageCache>> {
    IMAGE_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            create_image_cache().expect("Failed to initialize LRU image cache"),
        ))
    })
}

pub fn get_card_lookup() -> &'static Arc<RwLock<Option<CardNameLookup>>> {
    CARD_LOOKUP.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn get_search_results_cache() -> &'static Arc<RwLock<LruSearchCache>> {
    SEARCH_RESULTS_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            create_search_cache().expect("Failed to initialize LRU search results cache"),
        ))
    })
}

pub fn get_set_codes_cache() -> &'static Arc<RwLock<Option<HashSet<String>>>> {
    SET_CODES_CACHE.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn get_card_name_cache_info_ref() -> &'static Arc<RwLock<Option<(time::OffsetDateTime, usize)>>>
{
    CARD_NAME_CACHE_INFO.get_or_init(|| Arc::new(RwLock::new(None)))
}

// Eager initialization function - call at application startup
pub async fn initialize_caches() -> Result<(), ProxyError> {
    // Initialize image cache (loads from disk)
    let _image_cache = get_image_cache();
    info!("Image cache initialized at startup");

    // Initialize search results cache (loads from disk)
    let _search_cache = get_search_results_cache();
    info!("Search results cache initialized at startup");

    // Initialize card name lookup from disk at startup
    ensure_card_lookup_initialized().await?;

    // Initialize set codes from disk at startup
    ensure_set_codes_initialized().await?;

    Ok(())
}

// Save all in-memory caches to disk (without shutdown)
pub async fn save_caches() -> Result<(), ProxyError> {
    info!("Saving all in-memory caches to disk");

    // Save image cache metadata
    {
        let image_cache = get_image_cache();
        let cache_guard = image_cache.read().unwrap();
        cache_guard.save_to_storage()?;
        debug!("Image cache saved to disk");
    }

    // Save search results cache
    {
        let search_cache = get_search_results_cache();
        let cache_guard = search_cache.read().unwrap();
        cache_guard.save_to_storage()?;
        debug!("Search results cache saved to disk");
    }

    // Card names and set codes caches save immediately when updated from API
    // (no need to save - they only change when force-updated and save immediately)

    info!("All in-memory caches saved to disk successfully");
    Ok(())
}

// Shutdown function - save all caches to disk
pub async fn shutdown_caches() -> Result<(), ProxyError> {
    info!("Saving all caches to disk before shutdown");

    // Reuse the save logic
    save_caches().await?;

    info!("All caches saved to disk successfully");
    Ok(())
}

// Convenience functions - these now only check if already initialized
pub async fn ensure_card_lookup_initialized() -> Result<(), ProxyError> {
    let lookup_ref = get_card_lookup();
    let needs_init = {
        let lookup = lookup_ref.read().unwrap();
        lookup.is_none()
    };

    if needs_init {
        info!("Initializing CardNameLookup from disk cache");
        let client = get_scryfall_client();
        let cache = CardNameCache::new()?;

        // This will log disk cache operations internally
        let card_names = cache.get_card_names(client, false).await?;

        info!(
            card_count = card_names.names.len(),
            "Building fuzzy matching index from card names"
        );
        let start = std::time::Instant::now();
        let lookup = CardNameLookup::from_card_names(&card_names.names);
        let duration = start.elapsed();

        info!(
            duration_ms = duration.as_millis(),
            "CardNameLookup fuzzy index construction complete"
        );

        let mut lookup_guard = lookup_ref.write().unwrap();
        *lookup_guard = Some(lookup);

        // Store cache info in memory to avoid disk reads on every GUI frame
        let cache_info_ref = get_card_name_cache_info_ref();
        let mut cache_info_guard = cache_info_ref.write().unwrap();
        *cache_info_guard = card_names.date.map(|date| (date, card_names.names.len()));
    }

    Ok(())
}

pub async fn force_update_card_lookup() -> Result<(), ProxyError> {
    info!("Force updating CardNameLookup from Scryfall API");
    let client = get_scryfall_client();
    let cache = CardNameCache::new()?;

    // This will log the forced API fetch internally
    let card_names = cache.get_card_names(client, true).await?;

    info!(
        card_count = card_names.names.len(),
        "Building new fuzzy matching index from fresh data"
    );
    let start = std::time::Instant::now();
    let lookup = CardNameLookup::from_card_names(&card_names.names);
    let duration = start.elapsed();

    info!(
        duration_ms = duration.as_millis(),
        "Force update: CardNameLookup fuzzy index construction complete"
    );

    let lookup_ref = get_card_lookup();
    let mut lookup_guard = lookup_ref.write().unwrap();
    *lookup_guard = Some(lookup);

    // Update cache info in memory
    let cache_info_ref = get_card_name_cache_info_ref();
    let mut cache_info_guard = cache_info_ref.write().unwrap();
    *cache_info_guard = card_names.date.map(|date| (date, card_names.names.len()));

    Ok(())
}

pub fn find_card_name(name: &str) -> Option<NameLookupResult> {
    let lookup_ref = get_card_lookup();
    let lookup = lookup_ref.read().unwrap();
    let result = lookup.as_ref()?.find(name);

    match &result {
        Some(found) => debug!(input = %name, found = %found.name, "Fuzzy match found"),
        None => debug!(input = %name, "No fuzzy match found"),
    }

    result
}

pub async fn ensure_set_codes_initialized() -> Result<(), ProxyError> {
    let set_codes_ref = get_set_codes_cache();
    let needs_init = {
        let codes = set_codes_ref.read().unwrap();
        codes.is_none()
    };
    if needs_init {
        info!("Initializing set codes from disk cache");
        let client = get_scryfall_client();
        let cache = SetCodesCache::new()?;

        // This will log disk cache operations internally
        let set_codes = cache.get_set_codes(client, false).await?;

        info!(
            set_code_count = set_codes.codes.len(),
            "Loaded set codes into memory"
        );

        // Convert to HashSet for fast lookups
        let codes_set: HashSet<String> = set_codes.codes.into_iter().collect();

        {
            let mut cache_guard = set_codes_ref.write().unwrap();
            *cache_guard = Some(codes_set);
        }

        info!("Set codes initialization complete");
    }
    Ok(())
}

pub async fn force_update_set_codes() -> Result<(), ProxyError> {
    info!("Force updating set codes from Scryfall API");
    let client = get_scryfall_client();
    let cache = SetCodesCache::new()?;

    // This will log the forced API fetch internally
    let set_codes = cache.get_set_codes(client, true).await?;

    info!(
        set_code_count = set_codes.codes.len(),
        "Force update: Fresh set codes loaded from API"
    );

    // Convert to HashSet for fast lookups
    let codes_set: HashSet<String> = set_codes.codes.into_iter().collect();

    {
        let set_codes_ref = get_set_codes_cache();
        let mut cache_guard = set_codes_ref.write().unwrap();
        *cache_guard = Some(codes_set);
    }

    info!("Force update: Set codes cache updated");
    Ok(())
}

pub async fn get_or_fetch_image_bytes(url: &str) -> Result<Vec<u8>, ProxyError> {
    let cache = get_image_cache();
    let client = get_scryfall_client();

    // Try to get from cache first (note: this needs mutable access for LRU tracking)
    let cached_bytes = {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.get(&url.to_string())
    };

    match cached_bytes {
        Some(bytes) => Ok(bytes),
        None => {
            debug!(url = %url, "Image cache MISS, fetching from network");

            // Fetch raw bytes and cache them
            let raw_bytes = client.get_image_bytes(url).await?;

            // Insert raw bytes into cache (this handles disk persistence and LRU eviction)
            {
                let mut cache_guard = cache.write().unwrap();
                cache_guard.insert(url.to_string(), raw_bytes.clone())?;
            }

            Ok(raw_bytes)
        }
    }
}

/// Get or fetch image and convert to DynamicImage (for PDF generation)
pub async fn get_or_fetch_image(
    url: &str,
) -> Result<printpdf::image_crate::DynamicImage, ProxyError> {
    let raw_bytes = get_or_fetch_image_bytes(url).await?;

    // Convert raw bytes to DynamicImage at the point of use
    printpdf::image_crate::load_from_memory(&raw_bytes)
        .map_err(|e| ProxyError::Cache(format!("Failed to decode image: {}", e)))
}

pub fn get_card_name_cache_info() -> Option<(time::OffsetDateTime, usize)> {
    let cache_info_ref = get_card_name_cache_info_ref();
    let cache_info_guard = cache_info_ref.read().unwrap();
    cache_info_guard.clone()
}

/// Get card names cache statistics (count and estimated size in MB)
pub fn get_card_names_cache_size() -> Option<(usize, f64)> {
    let cache_info = get_card_name_cache_info()?;
    let count = cache_info.1; // Number of card names
    let estimated_bytes = (count as u64) * CARD_NAME_SIZE_ESTIMATE * FUZZY_INDEX_OVERHEAD_FACTOR;
    let size_mb = estimated_bytes as f64 / (1024.0 * 1024.0);
    Some((count, size_mb))
}

/// Get image cache statistics (count and size in MB)
pub fn get_image_cache_info() -> (usize, f64) {
    let cache = get_image_cache();
    let cache_guard = cache.read().unwrap();
    let count = cache_guard.len();
    let size_mb = cache_guard.size_bytes() as f64 / (1024.0 * 1024.0);
    (count, size_mb)
}

/// Get raw image bytes from cache for GUI display (returns None if not cached)
pub fn get_cached_image_bytes(url: &str) -> Option<Vec<u8>> {
    let cache = get_image_cache();
    let mut cache_guard = cache.write().unwrap();
    cache_guard.get(&url.to_string())
}

/// Get search results cache statistics (count and estimated size in MB)
pub fn get_search_results_cache_info() -> (usize, f64) {
    let cache = get_search_results_cache();
    let cache_guard = cache.read().unwrap();
    let count = cache_guard.len();
    let size_mb = cache_guard.size_bytes() as f64 / (1024.0 * 1024.0);
    (count, size_mb)
}

pub async fn get_or_fetch_search_results(
    card_name: &str,
) -> Result<crate::scryfall::CardSearchResult, ProxyError> {
    let client = get_scryfall_client();
    let cache = get_search_results_cache();

    // Check cache first (separate scope to release lock)
    let cached_result = {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.get(&card_name.to_lowercase())
    };

    if let Some(result) = cached_result {
        debug!(card_name = %card_name, "Search results cache HIT");
        return Ok(result);
    }

    // Cache miss - fetch from API
    debug!(card_name = %card_name, "Search results cache MISS, fetching from API");
    let search_results = client.search_card(card_name).await?;

    // Insert into cache (separate scope to release lock)
    {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.insert(card_name.to_lowercase(), search_results.clone())?;
        debug!(
            card_name = %card_name,
            results_count = search_results.cards.len(),
            "Search results cached"
        );
    }

    Ok(search_results)
}

/// Get the actual cache directory path
pub fn get_cache_directory_path() -> String {
    let cache_dir = ProjectDirs::from("", "", "magic-proxy")
        .map(|proj_dirs| proj_dirs.cache_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("magic-proxy-cache"));
    
    cache_dir.to_string_lossy().to_string()
}

/// Get the image cache directory path
pub fn get_image_cache_path() -> String {
    format!("{}/", get_cache_directory_path())
}

/// Get the search results cache file path
pub fn get_search_cache_path() -> String {
    format!("{}/search_results_cache.json", get_cache_directory_path())
}

/// Get the card names cache file path
pub fn get_card_names_cache_path() -> String {
    format!("{}/card_names.json", get_cache_directory_path())
}

/// Get the set codes cache file path
pub fn get_set_codes_cache_path() -> String {
    format!("{}/set_codes.json", get_cache_directory_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialize_caches() {
        // Should not panic and should return Ok
        let result = initialize_caches().await;
        assert!(result.is_ok());

        // Cache should be initialized and accessible
        let cache = get_image_cache();
        let cache_guard = cache.read().unwrap();
        // Should not panic when accessing cache methods
        let _size = cache_guard.len();
    }
}
