use crate::{
    CardNameCache, CardNameLookup, ImageCache, NameLookupResult, ProxyError, ScryfallClient,
};
use printpdf::image_crate::DynamicImage;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{info, debug};

// Global singletons - initialized once, shared everywhere
static SCRYFALL_CLIENT: OnceLock<ScryfallClient> = OnceLock::new();
static IMAGE_CACHE: OnceLock<Arc<RwLock<ImageCache>>> = OnceLock::new();
static CARD_LOOKUP: OnceLock<Arc<RwLock<Option<CardNameLookup>>>> = OnceLock::new();

pub fn get_scryfall_client() -> &'static ScryfallClient {
    SCRYFALL_CLIENT.get_or_init(|| ScryfallClient::new().expect("Failed to create ScryfallClient"))
}

pub fn get_image_cache() -> &'static Arc<RwLock<ImageCache>> {
    IMAGE_CACHE.get_or_init(|| Arc::new(RwLock::new(ImageCache::new())))
}

pub fn get_card_lookup() -> &'static Arc<RwLock<Option<CardNameLookup>>> {
    CARD_LOOKUP.get_or_init(|| Arc::new(RwLock::new(None)))
}

// Convenience functions
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
    } else {
        debug!("CardNameLookup already initialized");
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

pub async fn get_or_fetch_image(url: &str) -> Result<DynamicImage, ProxyError> {
    let cache = get_image_cache();
    let client = get_scryfall_client();

    let cached_image = (|| {
        let ca = cache.read().unwrap();
        ca.get(url).map(|x| x.clone())
    })();
    
    match cached_image {
        Some(image) => {
            debug!(url = %url, "Image cache HIT");
            Ok(image.clone())
        },
        None => {
            debug!(url = %url, "Image cache MISS, fetching from network");
            let new_image = client.get_image(url).await?;
            cache
                .write()
                .unwrap()
                .insert(url.to_string(), new_image.clone());
            debug!(url = %url, "Image cached");
            Ok(new_image)
        }
    }
}

pub fn get_card_name_cache_info() -> Option<(time::OffsetDateTime, usize)> {
    match CardNameCache::new() {
        Ok(cache) => cache.get_cache_info(),
        Err(_) => None,
    }
}
