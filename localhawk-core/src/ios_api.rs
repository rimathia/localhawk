//! iOS-specific sync API implementation
//! This module provides sync versions of async functions for iOS, using minimal duplication

#[cfg(feature = "ios")]
use crate::{
    decklist::DecklistEntry,
    error::ProxyError,
    globals::{get_image_cache, get_search_results_cache, get_set_codes_cache, find_card_name},
    http_client::{HttpClient, UreqHttpClient},
    lookup::NameMatchMode,
    scryfall::models::{Card, CardSearchResult},
    DoubleFaceMode,
};

/// iOS sync API implementation
#[cfg(feature = "ios")]
pub struct ProxyGenerator;

#[cfg(feature = "ios")]
impl ProxyGenerator {
    /// iOS sync version of search_card
    pub fn search_card_sync(name: &str) -> Result<CardSearchResult, ProxyError> {
        let client = UreqHttpClient::new()?;
        client.search_card(name)
    }

    /// iOS sync version of resolve_decklist_entries_to_cards  
    /// This does card search, selection, and automatically starts background loading
    pub fn resolve_decklist_entries_to_cards_sync(
        entries: &[DecklistEntry],
    ) -> Result<Vec<(Card, u32, DoubleFaceMode)>, ProxyError> {
        let mut card_list = Vec::new();

        for entry in entries {
            log::debug!("Searching for card: '{}'", entry.name);
            match Self::search_card_sync(&entry.name) {
                Ok(search_result) => {
                    // Use the same card selection logic as the main ProxyGenerator
                    let selected_card = search_result
                        .cards
                        .iter()
                        .position(|c| {
                            let name_matches = c.name.to_lowercase() == entry.name.to_lowercase();
                            let set_matches = if let Some(ref entry_set) = entry.set {
                                c.set.to_lowercase() == entry_set.to_lowercase()
                            } else {
                                true
                            };
                            let lang_matches = if let Some(ref entry_lang) = entry.lang {
                                c.language.to_lowercase() == entry_lang.to_lowercase()
                            } else {
                                true
                            };
                            name_matches && set_matches && lang_matches
                        })
                        .and_then(|idx| search_result.cards.get(idx))
                        .or_else(|| search_result.cards.first())
                        .cloned();

                    if let Some(card) = selected_card {
                        card_list.push((card, entry.multiple as u32, entry.face_mode.clone()));
                    }
                }
                Err(_) => {
                    // Skip cards that can't be found
                }
            }
        }

        // Automatically start background loading for resolved cards (like desktop)
        if !card_list.is_empty() {
            println!("🚀 iOS: Auto-starting background loading for {} resolved cards", card_list.len());
            
            // Start background loading in separate thread (fire and forget, like desktop)
            let cards_clone = card_list.clone();
            std::thread::spawn(move || {
                println!("🧵 iOS: Background loading thread started for {} resolved cards", cards_clone.len());
                
                for (card, quantity, face_mode) in &cards_clone {
                    // Cache images for each copy of the card
                    for _ in 0..*quantity {
                        let urls = card.get_images_for_face_mode(face_mode);
                        for url in urls {
                            match get_or_fetch_image_bytes_sync(&url) {
                                Ok(_) => {
                                    println!("✅ iOS: Cached resolved image: {}", url);
                                }
                                Err(e) => {
                                    println!("❌ iOS: Failed to cache resolved image {}: {:?}", url, e);
                                }
                            }
                        }
                    }
                }
                
                println!("✅ iOS: Background loading completed for resolved cards");
            });
        }

        Ok(card_list)
    }

    /// iOS sync version of parse_and_resolve_decklist
    /// Also starts background loading of alternative printings (fire and forget)
    pub fn parse_and_resolve_decklist_sync(
        decklist_text: &str,
        global_face_mode: DoubleFaceMode,
    ) -> Result<Vec<DecklistEntry>, ProxyError> {
        use crate::scryfall::models::get_minimal_scryfall_languages;
        use crate::decklist::parse_decklist;
        use crate::globals::find_card_name;

        // Get required data for parsing
        let scryfall_languages = get_minimal_scryfall_languages();
        let set_codes = {
            let set_codes_ref = crate::globals::get_set_codes_cache();
            let codes_guard = set_codes_ref.read().unwrap();
            codes_guard.as_ref().cloned().unwrap_or_default()
        };
        
        // Parse the decklist text using shared logic
        let parsed_lines = parse_decklist(decklist_text, &scryfall_languages, &set_codes);
        let parsed_entries: Vec<_> = parsed_lines.into_iter().filter_map(|line| line.as_entry()).collect();
        
        let mut resolved_entries = Vec::new();
        
        for mut entry in parsed_entries {
            log::debug!(
                "📝 iOS Parse: Processing '{}' [set: {:?}, lang: {:?}]",
                entry.name, entry.set, entry.lang
            );
            
            // Use shared business logic for name resolution (EXACTLY like desktop)
            if let Some(lookup_result) = find_card_name(&entry.name) {
                log::debug!(
                    "🔍 iOS Parse: Name resolved '{}' -> '{}' (keeping set: {:?}, lang: {:?})",
                    entry.name, lookup_result.name, entry.set, entry.lang
                );
                entry.name = lookup_result.name;
                // Apply face mode resolution logic (matches desktop logic)
                entry.face_mode = match lookup_result.hit {
                    NameMatchMode::Part(1) => {
                        DoubleFaceMode::BackOnly // Back face: always back only
                    }
                    _ => {
                        global_face_mode.clone() // Front face or full name: use global setting
                    }
                };
            } else {
                log::debug!("🔍 iOS Parse: No name resolution for '{}'", entry.name);
                entry.face_mode = global_face_mode.clone(); // No match: use global setting
            }
            
            log::debug!(
                "✅ iOS Parse: Final entry '{}' [set: {:?}, lang: {:?}, face_mode: {:?}]",
                entry.name, entry.set, entry.lang, entry.face_mode
            );
            resolved_entries.push(entry);
        }

        // Note: Background loading of alternative printings is handled separately in iOS
        // via the load_alternative_printings_sync function when needed

        Ok(resolved_entries)
    }

    /// iOS sync version of get_or_fetch_image_bytes
    pub fn get_or_fetch_image_bytes_sync(url: &str) -> Result<Vec<u8>, ProxyError> {
        let cache = get_image_cache();
        let client = UreqHttpClient::new()?;
        
        // Try to get from cache first (separate scope to release lock)
        let cached_bytes = {
            let mut cache_guard = cache.write().unwrap();
            cache_guard.get(&url.to_string())
        };
        
        if let Some(bytes) = cached_bytes {
            log::debug!("Image cache HIT for URL: {}", url);
            return Ok(bytes);
        }
        
        // Cache miss - fetch from API using sync client
        log::debug!("Image cache MISS for URL: {}, fetching...", url);
        let image_bytes = client.get_image_bytes(url)?;
        
        // Store in cache
        {
            let mut cache_guard = cache.write().unwrap();
            let _ = cache_guard.insert(url.to_string(), image_bytes.clone());
        }
        
        // Notify that image was cached
        #[cfg(feature = "ios")]
        {
            crate::ffi::queue_image_cache_notification(1, url); // 1 = ImageCached
            crate::ffi::notify_image_cache_dispatch_source();
        }
        
        Ok(image_bytes)
    }
    
    /// iOS sync version of generate_pdf_from_entries
    pub fn generate_pdf_from_entries_sync<F>(
        entries: &[DecklistEntry],
        options: crate::pdf::PdfOptions,
        mut progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize),
    {
        println!("🦀 [Rust] generate_pdf_from_entries_sync received {} entries:", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            let set_str = entry.set.as_deref().unwrap_or("any");
            let lang_str = entry.lang.as_deref().unwrap_or("any");
            println!("  [{}] '{}' ({}) [{}] x{} face={:?}", i, entry.name, set_str, lang_str, entry.multiple, entry.face_mode);
        }

        // Convert entries to cards using sync API
        let cards = Self::resolve_decklist_entries_to_cards_sync(entries)?;

        println!("🦀 [Rust] resolved to {} cards:", cards.len());
        for (i, (card, qty, face_mode)) in cards.iter().enumerate() {
            println!("  [{}] '{}' ({}) [{}] x{} face={:?}", i, card.name, card.set, card.language, qty, face_mode);
        }

        // Use existing PDF generation logic (mostly pure, just needs sync image fetching)
        Self::generate_pdf_from_cards_with_face_modes_sync(&cards, options, progress_callback)
    }
    
    /// iOS sync version of generate_pdf_from_cards_with_face_modes
    pub fn generate_pdf_from_cards_with_face_modes_sync<F>(
        cards: &[(Card, u32, DoubleFaceMode)],
        options: crate::pdf::PdfOptions,
        mut progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize),
    {
        use crate::pdf::generate_pdf;
        
        // Expand cards to image URLs using shared logic from main ProxyGenerator
        let image_urls = crate::ProxyGenerator::expand_cards_to_image_urls(cards);

        println!("🦀 [Rust] PDF expansion generated {} image URLs:", image_urls.len());
        for (i, url) in image_urls.iter().enumerate() {
            println!("  [{}] {}", i, url);
        }

        let total_images = image_urls.len();
        progress_callback(0, total_images);
        
        // Download images sequentially (sync version)
        let mut images = Vec::new();
        for (i, url) in image_urls.iter().enumerate() {
            let image_bytes = Self::get_or_fetch_image_bytes_sync(url)?;
            
            // Convert bytes to DynamicImage
            let image = image::load_from_memory(&image_bytes)
                .map_err(|e| ProxyError::InvalidCard(format!("Failed to decode image: {}", e)))?;
            
            images.push(image);
            progress_callback(i + 1, total_images);
        }
        
        // Generate PDF using shared logic
        generate_pdf(images.into_iter(), options)
    }
    
    /// iOS sync version of clear_cache
    pub fn clear_cache_sync() -> Result<(), ProxyError> {
        let cache = get_image_cache();
        let mut cache_guard = cache.write().unwrap();
        let _ = cache_guard.clear();
        Ok(())
    }
    
    /// iOS sync version of get_entry_count  
    pub fn get_entry_count_sync() -> usize {
        // This is just a read operation - same implementation
        crate::globals::get_image_cache().read().unwrap().len()
    }
    
    /// iOS sync version to ensure card lookup is initialized  
    pub fn ensure_card_lookup_initialized_sync() -> Result<(), ProxyError> {
        let lookup_ref = crate::globals::get_card_lookup();
        let needs_init = {
            let lookup = lookup_ref.read().unwrap();
            lookup.is_none()
        };

        if needs_init {
            log::info!("Initializing CardNameLookup using sync iOS cache");
            let client = UreqHttpClient::new()?;
            
            // Use the new sync cache implementation with pure business logic
            let (lookup, cache_info) = crate::ios_cache::initialize_card_lookup_sync(&client)?;

            let mut lookup_guard = lookup_ref.write().unwrap();
            *lookup_guard = Some(lookup);

            // Store cache info in memory to avoid disk reads on every GUI frame
            let cache_info_ref = crate::globals::get_card_name_cache_info_ref();
            let mut cache_info_guard = cache_info_ref.write().unwrap();
            *cache_info_guard = cache_info;
        }

        Ok(())
    }
    
    /// iOS sync version to ensure set codes are initialized
    pub fn ensure_set_codes_initialized_sync() -> Result<(), ProxyError> {
        let set_codes_ref = get_set_codes_cache();
        let needs_init = {
            let codes = set_codes_ref.read().unwrap();
            codes.is_none()
        };
        
        if needs_init {
            log::info!("Initializing set codes using sync iOS cache");
            let client = UreqHttpClient::new()?;
            
            // Use the new sync cache implementation with pure business logic
            let codes_set = crate::ios_cache::initialize_set_codes_sync(&client)?;

            {
                let mut cache_guard = set_codes_ref.write().unwrap();
                *cache_guard = Some(codes_set);
            }

            log::info!("Set codes initialization complete");
        }
        
        Ok(())
    }
}

/// iOS sync version of get_or_fetch_search_results (standalone function)
#[cfg(feature = "ios")]
pub fn get_or_fetch_search_results_sync(name: &str) -> Result<CardSearchResult, ProxyError> {
    let cache = get_search_results_cache();
    let client = UreqHttpClient::new()?;
    
    // Try to get from cache first (separate scope to release lock)
    let cached_result = {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.get(&name.to_string())
    };
    
    if let Some(result) = cached_result {
        log::debug!("Search cache HIT for name: {}", name);
        return Ok(result);
    }
    
    // Cache miss - fetch from API using sync client
    log::debug!("Search cache MISS for name: {}, fetching...", name);
    let search_result = client.search_card(name)?;
    
    // Store in cache
    {
        let mut cache_guard = cache.write().unwrap();
        let _ = cache_guard.insert(name.to_string(), search_result.clone());
    }
    
    Ok(search_result)
}

/// iOS sync version of get_or_fetch_image_bytes (standalone function)
#[cfg(feature = "ios")]
pub fn get_or_fetch_image_bytes_sync(url: &str) -> Result<Vec<u8>, ProxyError> {
    let cache = get_image_cache();
    let client = UreqHttpClient::new()?;
    
    // Try to get from cache first (separate scope to release lock)
    let cached_bytes = {
        let mut cache_guard = cache.write().unwrap();
        cache_guard.get(&url.to_string())
    };
    
    if let Some(bytes) = cached_bytes {
        log::debug!("Image cache HIT for URL: {}", url);
        return Ok(bytes);
    }
    
    // Cache miss - fetch from API using sync client
    log::debug!("Image cache MISS for URL: {}, fetching...", url);
    let image_bytes = client.get_image_bytes(url)?;
    
    // Store in cache
    {
        let mut cache_guard = cache.write().unwrap();
        let _ = cache_guard.insert(url.to_string(), image_bytes.clone());
    }
    
    // Notify that image was cached
    #[cfg(feature = "ios")]
    {
        crate::ffi::queue_image_cache_notification(1, url); // 1 = ImageCached
        crate::ffi::notify_image_cache_dispatch_source();
    }
    
    Ok(image_bytes)
}

impl ProxyGenerator {
    /// Load all printings for all entries (iOS sync version of Phase 2)
    /// This should be called after parsing to populate the print selection modal with cached images
    pub fn load_alternative_printings_sync(entries: &[DecklistEntry]) -> Result<usize, ProxyError> {
        let mut images_loaded = 0;
        
        println!("🔄 [iOS API] Starting all printings loading for {} entries", entries.len());
        
        for (entry_idx, entry) in entries.iter().enumerate() {
            println!("🔍 [iOS API] Loading all printings for entry {}/{}: '{}'", 
                entry_idx + 1, entries.len(), entry.name);
            
            // Search for all available printings
            match Self::search_card_sync(&entry.name) {
                Ok(search_result) => {
                    println!("  Found {} total printings", search_result.cards.len());
                    
                    // Load all printings (cache will handle duplicates efficiently)
                    for card in &search_result.cards {
                        // Load front image for each printing
                        match get_or_fetch_image_bytes_sync(&card.border_crop) {
                            Ok(_) => {
                                images_loaded += 1;
                                println!("  ✅ Cached printing: '{}' ({}) [{}]", 
                                    card.name, card.set.to_uppercase(), card.language);
                            }
                            Err(e) => {
                                println!("  ❌ Failed to cache printing '{}' ({}): {}", 
                                    card.name, card.set.to_uppercase(), e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("  ❌ Failed to search for printings of '{}': {}", entry.name, e);
                }
            }
        }
        
        println!("✅ [iOS API] All printings loading complete: {} images processed", images_loaded);
        Ok(images_loaded)
    }
}

