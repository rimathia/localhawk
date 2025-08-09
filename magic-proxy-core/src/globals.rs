use crate::{
    CardNameCache, CardNameLookup, ImageCache, NameLookupResult, ProxyError, 
    ScryfallClient, SearchResultsCache, SetCodesCache,
};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{info, debug};

// Global singletons - initialized once, shared everywhere
static SCRYFALL_CLIENT: OnceLock<ScryfallClient> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<ImageCache>>> = OnceLock::new();
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();
static SEARCH_RESULTS_CACHE: OnceLock<Arc<RwLock<SearchResultsCache>>> = OnceLock::new();
static SET_CODES_CACHE: OnceLock<Arc<RwLock<Option<HashSet<String>>>>> = OnceLock::new();
static CARD_NAME_CACHE_INFO: OnceLock<Arc<RwLock<Option<(time::OffsetDateTime, usize)>>>> = OnceLock::new();

pub fn get_scryfall_client() -> &'static ScryfallClient {
    SCRYFALL_CLIENT.get_or_init(|| ScryfallClient::new().expect("Failed to create ScryfallClient"))
}

pub fn get_image_cache() -> &'static Arc<RwLock<ImageCache>> {
    IMAGE_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            ImageCache::new().expect("Failed to initialize image cache")
        ))
    })
}

pub fn get_card_lookup() -> &'static Arc<RwLock<Option<CardNameLookup>>> {
    CARD_LOOKUP.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn get_search_results_cache() -> &'static Arc<RwLock<SearchResultsCache>> {
    SEARCH_RESULTS_CACHE.get_or_init(|| {
        Arc::new(RwLock::new(
            SearchResultsCache::new().expect("Failed to initialize search results cache")
        ))
    })
}

pub fn get_set_codes_cache() -> &'static Arc<RwLock<Option<HashSet<String>>>> {
    SET_CODES_CACHE.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn get_card_name_cache_info_ref() -> &'static Arc<RwLock<Option<(time::OffsetDateTime, usize)>>> {
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

// Shutdown function - save all caches to disk
pub async fn shutdown_caches() -> Result<(), ProxyError> {
    info!("Saving all caches to disk before shutdown");
    
    // Save image cache metadata
    {
        let image_cache = get_image_cache();
        let cache_guard = image_cache.read().unwrap();
        cache_guard.save_to_disk()?;
        info!("Image cache metadata saved to disk");
    }
    
    // Save search results cache
    {
        let search_cache = get_search_results_cache();
        let cache_guard = search_cache.read().unwrap();
        cache_guard.save_to_disk()?;
        info!("Search results cache saved to disk");
    }
    
    // Card names and set codes caches save immediately when updated from API
    // (no need to save at shutdown - they only change when force-updated)
    
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
        cache_guard.get(url)
    };
    
    match cached_bytes {
        Some(bytes) => {
            Ok(bytes)
        },
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
pub async fn get_or_fetch_image(url: &str) -> Result<printpdf::image_crate::DynamicImage, ProxyError> {
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

/// Get image cache statistics (count and size in MB)
pub fn get_image_cache_info() -> (usize, f64) {
    let cache = get_image_cache();
    let cache_guard = cache.read().unwrap();
    let count = cache_guard.size();
    let size_mb = cache_guard.size_bytes() as f64 / (1024.0 * 1024.0);
    (count, size_mb)
}

/// Get raw image bytes from cache for GUI display (returns None if not cached)
pub fn get_cached_image_bytes(url: &str) -> Option<Vec<u8>> {
    let cache = get_image_cache();
    let mut cache_guard = cache.write().unwrap();
    cache_guard.get(url)
}

pub async fn get_or_fetch_search_results(card_name: &str) -> Result<crate::scryfall::CardSearchResult, ProxyError> {
    let client = get_scryfall_client();
    let cache = get_search_results_cache();
    
    // Check cache first (separate scope to release lock)
    let cached_result = {
        let mut cache_guard = cache.write().unwrap();
        if let Some(cached) = cache_guard.cache.get_mut(&card_name.to_lowercase()) {
            cached.last_accessed = time::OffsetDateTime::now_utc();
            debug!(card_name = %card_name, "Search results cache HIT");
            Some(cached.search_results.clone())
        } else {
            None
        }
    };
    
    if let Some(result) = cached_result {
        return Ok(result);
    }
    
    // Cache miss - fetch from API
    debug!(card_name = %card_name, "Search results cache MISS, fetching from API");
    let search_results = client.search_card(card_name).await?;
    
    // Insert into cache (separate scope to release lock)
    {
        let mut cache_guard = cache.write().unwrap();
        let cached_result = crate::search_results_cache::CachedSearchResult {
            card_name: card_name.to_lowercase(),
            search_results: search_results.clone(),
            cached_at: time::OffsetDateTime::now_utc(),
            last_accessed: time::OffsetDateTime::now_utc(),
        };
        
        cache_guard.cache.insert(card_name.to_lowercase(), cached_result);
        // Cache will be saved to disk at shutdown
        
        debug!(
            card_name = %card_name,
            results_count = search_results.cards.len(),
            "Search results cached to disk"
        );
    }
    
    Ok(search_results)
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
        let _size = cache_guard.size();
    }
}
