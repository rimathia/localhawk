pub mod background_loading;
pub mod cache;
pub mod cache_logic;
pub mod card_name_cache;
pub mod decklist;
pub mod error;
#[cfg(feature = "ios")]
pub mod ffi;
pub mod format;
pub mod globals;
#[cfg(feature = "ios")]
pub mod http_client;
#[cfg(feature = "ios")]
pub mod ios_api;
#[cfg(feature = "ios")]
pub mod ios_cache;
pub mod layout;
pub mod lookup;
pub mod pagination;
pub mod pdf;
pub mod scryfall;
pub mod search_results_cache;
pub mod set_codes_cache;

pub use background_loading::{
    BackgroundLoadHandle, BackgroundLoadProgress, LoadingPhase, start_background_image_loading,
};
pub use cache::{LruImageCache, LruSearchCache};
pub use card_name_cache::CardNameCache;
pub use set_codes_cache::SetCodesCache;

/// Face mode for double-faced cards - moved from pdf module as it's used throughout the codebase
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoubleFaceMode {
    /// Include only the front face of double-faced cards
    FrontOnly,
    /// Include only the back face of double-faced cards  
    BackOnly,
    /// Include both front and back faces as separate cards
    BothSides,
}

impl std::fmt::Display for DoubleFaceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoubleFaceMode::FrontOnly => write!(f, "Front face only"),
            DoubleFaceMode::BackOnly => write!(f, "Back face only"),
            DoubleFaceMode::BothSides => write!(f, "Both sides"),
        }
    }
}

impl DoubleFaceMode {
    pub fn all() -> Vec<DoubleFaceMode> {
        vec![
            DoubleFaceMode::FrontOnly,
            DoubleFaceMode::BackOnly,
            DoubleFaceMode::BothSides,
        ]
    }
}
pub use decklist::{DecklistEntry, ParsedDecklistLine, parse_decklist, parse_line};
pub use error::ProxyError;
pub use format::{build_aligned_parsed_output, format_decklist_entry, format_entries_summary};
pub use globals::{
    find_card_name, force_update_card_lookup, force_update_set_codes, get_cache_directory_path,
    get_cached_image_bytes, get_card_lookup, get_card_name_cache_info,
    get_card_name_cache_info_ref, get_card_names_cache_path, get_card_names_cache_size,
    get_image_cache, get_image_cache_info, get_image_cache_path, get_or_fetch_image,
    get_or_fetch_image_bytes, get_or_fetch_search_results, get_scryfall_client,
    get_search_cache_path, get_search_results_cache_info, get_set_codes_cache,
    get_set_codes_cache_path, initialize_caches, save_caches, shutdown_caches,
};
pub use layout::{GridImage, GridPosition, GridPreview, PageNavigation, PreviewEntry};
pub use lookup::{CardNameLookup, NameLookupResult, NameMatchMode};
pub use pagination::{PaginatedGrid, PaginatedView};
pub use pdf::{PageSize, PdfOptions, generate_pdf};
pub use scryfall::{
    Card, CardSearchResult, ScryfallCardNames, ScryfallClient,
    models::{ScryfallSetCodes, get_minimal_scryfall_languages},
};

/// Main interface for generating Magic card proxy sheets
#[derive(Debug)]
pub struct ProxyGenerator {
    cards: Vec<(Card, u32)>, // Only local state for this operation
}

impl ProxyGenerator {
    /// Create a new ProxyGenerator instance
    pub fn new() -> Result<Self, ProxyError> {
        Ok(ProxyGenerator { cards: Vec::new() })
    }

    /// Search for cards by name (uses cached results)
    pub async fn search_card(name: &str) -> Result<CardSearchResult, ProxyError> {
        get_or_fetch_search_results(name).await
    }

    /// Get all card names from Scryfall and initialize fuzzy matching (now uses global state)
    pub async fn initialize_card_lookup() -> Result<(), ProxyError> {
        // This is now handled by initialize_caches() at startup
        Ok(())
    }

    /// Force update card names from Scryfall and reinitialize fuzzy matching (now uses global state)
    pub async fn force_update_card_lookup() -> Result<(), ProxyError> {
        force_update_card_lookup().await
    }

    /// Find a card name using fuzzy matching (now uses global state)
    pub fn find_card_name(name: &str) -> Option<NameLookupResult> {
        find_card_name(name)
    }

    /// Parse a decklist and resolve card names using fuzzy matching with global face mode
    pub async fn parse_and_resolve_decklist(
        decklist_text: &str,
        global_face_mode: DoubleFaceMode,
    ) -> Result<Vec<DecklistEntry>, ProxyError> {
        use scryfall::models::get_minimal_scryfall_languages;

        // These should already be initialized at startup, just verify
        if get_card_lookup().read().unwrap().is_none() {
            return Err(ProxyError::Cache(
                "Card lookup not initialized - call initialize_caches() at startup".to_string(),
            ));
        }
        if get_set_codes_cache().read().unwrap().is_none() {
            return Err(ProxyError::Cache(
                "Set codes not initialized - call initialize_caches() at startup".to_string(),
            ));
        }

        let languages = get_minimal_scryfall_languages();

        // Get set codes from global cache
        let set_codes = {
            let set_codes_ref = crate::globals::get_set_codes_cache();
            let codes_guard = set_codes_ref.read().unwrap();
            codes_guard.as_ref().cloned().unwrap_or_default()
        };

        let parsed_lines = parse_decklist(decklist_text, &languages, &set_codes);

        let mut resolved_entries = Vec::new();
        for line in parsed_lines {
            if let Some(mut entry) = line.as_entry() {
                log::debug!(
                    "Processing entry: {}x '{}' [set: {:?}, lang: {:?}]",
                    entry.multiple,
                    entry.name,
                    entry.set,
                    entry.lang
                );
                // Try to resolve the card name using global fuzzy matching
                if let Some(lookup_result) = find_card_name(&entry.name) {
                    log::debug!(
                        "Name resolution: '{}' -> '{}' (face mode: {:?})",
                        entry.name,
                        lookup_result.name,
                        lookup_result.hit
                    );
                    entry.name = lookup_result.name;
                    // Apply face mode resolution logic (matches MagicHawk logic)
                    entry.face_mode = match lookup_result.hit {
                        crate::lookup::NameMatchMode::Part(1) => {
                            log::debug!("Back face input detected, using BackOnly mode");
                            DoubleFaceMode::BackOnly // Back face: always back only
                        }
                        _ => {
                            log::debug!(
                                "Front face or full name input, using global setting: {:?}",
                                global_face_mode
                            );
                            global_face_mode.clone() // Front face or full name: use global setting
                        }
                    };
                } else {
                    log::debug!(
                        "Name resolution: '{}' -> no match found, using global setting",
                        entry.name
                    );
                    entry.face_mode = global_face_mode.clone(); // No match: use global setting
                }
                resolved_entries.push(entry);
            }
        }

        log::debug!(
            "Final resolved decklist: {} entries",
            resolved_entries.len()
        );
        for entry in &resolved_entries {
            log::debug!(
                "  -> {}x '{}' [set: {:?}, lang: {:?}, face_mode: {:?}]",
                entry.multiple,
                entry.name,
                entry.set,
                entry.lang,
                entry.face_mode
            );
        }
        Ok(resolved_entries)
    }

    /// Add a card to the generation queue
    pub fn add_card(&mut self, card: Card, quantity: u32) {
        self.cards.push((card, quantity));
    }

    /// Remove a card from the generation queue by index
    pub fn remove_card(&mut self, index: usize) {
        if index < self.cards.len() {
            self.cards.remove(index);
        }
    }

    /// Get the current list of cards to be generated
    pub fn get_cards(&self) -> &[(Card, u32)] {
        &self.cards
    }

    /// Clear all cards from the generation queue
    pub fn clear_cards(&mut self) {
        self.cards.clear();
    }

    /// Generate PDF with progress callback
    pub async fn generate_pdf<F>(
        &mut self,
        options: PdfOptions,
        mut progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize) + Send,
    {
        if self.cards.is_empty() {
            return Err(ProxyError::InvalidCard("No cards to generate".to_string()));
        }

        // Calculate total images needed
        let total_images: usize = self.cards.iter().map(|(_, qty)| *qty as usize).sum();
        let mut current_progress = 0;

        // Collect all images
        let mut images = Vec::new();

        for (card, quantity) in &self.cards {
            for _ in 0..*quantity {
                progress_callback(current_progress, total_images);

                // Get image URLs for this card based on the face mode
                let image_urls = card.get_images_for_face_mode(&options.double_face_mode);

                for image_url in image_urls {
                    let image = get_or_fetch_image(&image_url).await?;
                    images.push(image);
                }

                current_progress += 1;
            }
        }

        progress_callback(total_images, total_images);

        // Generate PDF
        generate_pdf(images.into_iter(), options)
    }

    /// Get the image URLs that should be used for a given card and face mode
    /// This is the core logic extracted from PDF generation for reuse in grid preview
    pub fn get_image_urls_for_face_mode(card: &Card, face_mode: &DoubleFaceMode) -> Vec<String> {
        card.get_images_for_face_mode(face_mode)
    }

    /// Expand a list of cards with quantities into a sequential list of image URLs
    /// This is the single source of truth for what images appear in the PDF and in what order
    pub fn expand_cards_to_image_urls(cards: &[(Card, u32, DoubleFaceMode)]) -> Vec<String> {
        let mut image_urls = Vec::new();

        for (card, quantity, face_mode) in cards {
            for _ in 0..*quantity {
                let urls = card.get_images_for_face_mode(face_mode);
                image_urls.extend(urls);
            }
        }

        image_urls
    }

    /// Convert decklist entries to cards ready for PDF generation
    /// This is the shared logic for both PDF generation and grid preview
    pub async fn resolve_decklist_entries_to_cards(
        entries: &[DecklistEntry],
    ) -> Result<Vec<(Card, u32, DoubleFaceMode)>, ProxyError> {
        let mut card_list = Vec::new();

        for entry in entries {
            log::debug!("Searching for card: '{}'", entry.name);
            match Self::search_card(&entry.name).await {
                Ok(search_result) => {
                    log::debug!(
                        "Found {} printings for '{}'",
                        search_result.cards.len(),
                        entry.name
                    );

                    // Use the same card selection logic as used in both PDF generation and grid preview
                    let selected_card = search_result
                        .cards
                        .iter()
                        .position(|c| {
                            // First check if the card name matches what we're looking for
                            let name_matches = c.name.to_lowercase() == entry.name.to_lowercase();

                            // Try to match both set and language if specified
                            let set_matches = if let Some(ref entry_set) = entry.set {
                                c.set.to_lowercase() == entry_set.to_lowercase()
                            } else {
                                true // No set filter
                            };

                            let lang_matches = if let Some(ref entry_lang) = entry.lang {
                                c.language.to_lowercase() == entry_lang.to_lowercase()
                            } else {
                                true // No language filter
                            };

                            name_matches && set_matches && lang_matches
                        })
                        .and_then(|idx| search_result.cards.get(idx))
                        .or_else(|| search_result.cards.first())
                        .cloned();

                    if let Some(card) = selected_card {
                        log::debug!(
                            "Selected card: '{}' ({}) [{}] with face mode {:?}",
                            card.name,
                            card.set.to_uppercase(),
                            card.language,
                            entry.face_mode
                        );
                        card_list.push((card, entry.multiple as u32, entry.face_mode.clone()));
                    } else {
                        log::warn!("No suitable card found for entry '{}'", entry.name);
                    }
                }
                Err(e) => {
                    log::debug!("Failed to search for card '{}': {:?}", entry.name, e);
                    // Skip cards that can't be found - this matches current behavior
                }
            }
        }

        Ok(card_list)
    }

    /// Parse decklist and start background image loading (fire and forget)
    /// This function parses the decklist, kicks off background loading for all cards,
    /// and returns immediately. Background loading happens asynchronously.
    pub async fn parse_and_start_background_loading(
        decklist_text: &str,
        global_face_mode: DoubleFaceMode,
    ) -> Result<Vec<DecklistEntry>, ProxyError> {
        // First parse the decklist
        let entries = Self::parse_and_resolve_decklist(decklist_text, global_face_mode).await?;

        // Start background loading for all entries (fire and forget)
        if !entries.is_empty() {
            let entries_clone = entries.clone();
            let entry_count = entries.len();
            println!("About to spawn background loading task for {} entries", entry_count);
            tokio::spawn(async move {
                println!("Background loading task started for {} entries", entry_count);
                let _handle = start_background_image_loading(entries_clone);
                println!("Background loading task completed for {} entries", entry_count);
                // We don't wait for completion - just let it run in the background
                log::debug!(
                    "Background image loading started for {} entries",
                    entry_count
                );
            });
            println!("tokio::spawn called successfully");
        } else {
            println!("No entries to load in background");
        }

        // Return parsed entries immediately
        Ok(entries)
    }

    /// Generate PDF directly from decklist entries (highest level convenience method)
    pub async fn generate_pdf_from_entries<F>(
        entries: &[DecklistEntry],
        options: PdfOptions,
        progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize) + Send,
    {
        let cards = Self::resolve_decklist_entries_to_cards(entries).await?;
        Self::generate_pdf_from_cards_with_face_modes(&cards, options, progress_callback).await
    }

    /// Generate PDF from a list of cards with per-card face mode (static method using global state)
    pub async fn generate_pdf_from_cards_with_face_modes<F>(
        cards: &[(Card, u32, DoubleFaceMode)],
        options: PdfOptions,
        mut progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize) + Send,
    {
        if cards.is_empty() {
            return Err(ProxyError::InvalidCard("No cards to generate".to_string()));
        }

        // Use shared expansion logic to get the exact sequence of image URLs
        let image_urls = Self::expand_cards_to_image_urls(cards);
        let total_images = image_urls.len();

        // Download all images in sequence
        let mut images = Vec::new();
        for (current_progress, image_url) in image_urls.iter().enumerate() {
            progress_callback(current_progress, total_images);
            let image = get_or_fetch_image(image_url).await?;
            images.push(image);
        }

        progress_callback(total_images, total_images);

        // Generate PDF
        generate_pdf(images.into_iter(), options)
    }

    /// Generate PDF from a list of cards (static method using global state)
    pub async fn generate_pdf_from_cards<F>(
        cards: &[(Card, u32)],
        options: PdfOptions,
        mut progress_callback: F,
    ) -> Result<Vec<u8>, ProxyError>
    where
        F: FnMut(usize, usize) + Send,
    {
        if cards.is_empty() {
            return Err(ProxyError::InvalidCard("No cards to generate".to_string()));
        }

        // Calculate total images needed
        let total_images: usize = cards.iter().map(|(_, qty)| *qty as usize).sum();
        let mut current_progress = 0;

        // Collect all images
        let mut images = Vec::new();

        for (card, quantity) in cards {
            for _ in 0..*quantity {
                progress_callback(current_progress, total_images);

                // Get image URLs for this card (both front and back if exists)
                let image_urls = card.get_images_for_face_mode(&DoubleFaceMode::BothSides);

                for image_url in image_urls {
                    let image = get_or_fetch_image(&image_url).await?;
                    images.push(image);
                }

                current_progress += 1;
            }
        }

        progress_callback(total_images, total_images);

        // Generate PDF
        generate_pdf(images.into_iter(), options)
    }

    /// Get cache statistics (now uses global cache)
    pub fn cache_size() -> usize {
        let cache = get_image_cache();
        let cache_guard = cache.read().unwrap();
        cache_guard.len()
    }

    /// Clear the image cache (now uses global cache)
    pub fn clear_cache() -> Result<(), ProxyError> {
        let cache = get_image_cache();
        let mut cache_guard = cache.write().unwrap();
        cache_guard.clear()
    }

    /// Force evict a specific image from cache
    pub fn force_evict_image(url: &str) -> Result<(), ProxyError> {
        let cache = get_image_cache();
        let mut cache_guard = cache.write().unwrap();
        cache_guard.evict(&url.to_string()).map(|_| ())
    }

    /// Get card name cache information (timestamp and count) (now uses global function)
    pub fn get_card_name_cache_info() -> Option<(time::OffsetDateTime, usize)> {
        get_card_name_cache_info()
    }

    /// Clear the card name cache (now uses global state)
    pub fn clear_card_name_cache() -> Result<(), ProxyError> {
        let cache = CardNameCache::new()?;
        cache.clear_cache()
    }
}

impl Default for ProxyGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to create ProxyGenerator")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_search_card() {
        // Test searching for a well-known card
        let result = ProxyGenerator::search_card("Lightning Bolt").await;

        match result {
            Ok(search_result) => {
                assert!(
                    search_result.cards.len() > 0,
                    "Should find Lightning Bolt printings"
                );
                println!(
                    "Found {} Lightning Bolt printings",
                    search_result.cards.len()
                );
            }
            Err(e) => {
                println!(
                    "Search failed (this might be expected if no internet): {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_card_management() {
        let mut generator = ProxyGenerator::new().expect("Failed to create generator");

        // Create a dummy card
        let card = Card {
            name: "test card".to_string(),
            set: "test".to_string(),
            language: "en".to_string(),
            border_crop: "http://example.com/test.jpg".to_string(),
            back_side: None,
        };

        // Test adding card
        generator.add_card(card.clone(), 4);
        assert_eq!(generator.get_cards().len(), 1);
        assert_eq!(generator.get_cards()[0].1, 4);

        // Test removing card
        generator.remove_card(0);
        assert_eq!(generator.get_cards().len(), 0);

        // Test clearing cards
        generator.add_card(card, 2);
        generator.clear_cards();
        assert_eq!(generator.get_cards().len(), 0);
    }

    #[test]
    fn test_pdf_options() {
        let options = PdfOptions::default();
        assert_eq!(options.cards_per_row, 3);
        assert_eq!(options.cards_per_column, 3);
    }

    #[test]
    fn test_generator_cache_operations() {
        // Clear cache to ensure clean test state
        ProxyGenerator::clear_cache().unwrap();

        // Test cache is now empty
        assert_eq!(get_image_cache().read().unwrap().len(), 0);

        // Test clear cache (should not panic on empty cache)
        ProxyGenerator::clear_cache().unwrap();
        assert_eq!(get_image_cache().read().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_pdf_generation_empty_cards() {
        let mut generator = ProxyGenerator::new().expect("Failed to create generator");

        // Try to generate PDF with no cards
        let result = generator
            .generate_pdf(PdfOptions::default(), |_, _| {})
            .await;

        assert!(result.is_err());
        match result {
            Err(ProxyError::InvalidCard(msg)) => {
                assert_eq!(msg, "No cards to generate");
            }
            _ => panic!("Expected InvalidCard error"),
        }
    }

    #[tokio::test]
    async fn test_search_empty_string() {
        // Search for empty string should still work (might return error from API)
        let result = ProxyGenerator::search_card("").await;
        // This might succeed or fail depending on Scryfall API behavior
        // The important thing is it doesn't panic
        println!("Empty search result: {:?}", result);
    }

    #[test]
    fn test_card_removal_out_of_bounds() {
        let mut generator = ProxyGenerator::new().expect("Failed to create generator");

        // Try to remove card from empty list
        generator.remove_card(0);
        assert_eq!(generator.get_cards().len(), 0);

        // Add a card and try to remove with invalid index
        let card = Card {
            name: "test".to_string(),
            set: "test".to_string(),
            language: "en".to_string(),
            border_crop: "http://example.com/test.jpg".to_string(),
            back_side: None,
        };
        generator.add_card(card, 1);

        // Try to remove with out-of-bounds index
        generator.remove_card(10);
        assert_eq!(generator.get_cards().len(), 1); // Should still have the card

        // Remove with valid index
        generator.remove_card(0);
        assert_eq!(generator.get_cards().len(), 0);
    }

    #[tokio::test]
    async fn test_generator_default_creation() {
        // Clear cache to ensure clean test state
        ProxyGenerator::clear_cache().unwrap();

        // Test that default creation works
        let generator = ProxyGenerator::default();
        assert_eq!(generator.get_cards().len(), 0);
        assert_eq!(get_image_cache().read().unwrap().len(), 0);
    }

    #[test]
    fn test_fuzzy_card_name_lookup() {
        // Test the card name lookup functionality
        let card_names = vec![
            "Lightning Bolt".to_string(),
            "Brainstorm".to_string(),
            "Cut // Ribbons".to_string(),
        ];
        let lookup = CardNameLookup::from_card_names(&card_names);

        // Test exact match
        assert_eq!(
            lookup.find("lightning bolt"),
            Some(NameLookupResult {
                name: "lightning bolt".to_string(),
                hit: NameMatchMode::Full
            })
        );

        // Test partial match for split card
        assert_eq!(
            lookup.find("cut"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(0)
            })
        );

        // Test second part of split card
        assert_eq!(
            lookup.find("ribbons"),
            Some(NameLookupResult {
                name: "cut // ribbons".to_string(),
                hit: NameMatchMode::Part(1)
            })
        );
    }

    #[test]
    fn test_parse_and_resolve_decklist_face_preferences() {
        // Create a minimal card names lookup with just the cards we need for testing
        let test_card_names = vec![
            "Kabira Takedown // Kabira Plateau".to_string(),
            "Cut // Ribbons".to_string(),
        ];
        let lookup = CardNameLookup::from_card_names(&test_card_names);

        // Test ALL double face modes to ensure proper resolution
        let test_global_modes = [
            DoubleFaceMode::FrontOnly,
            DoubleFaceMode::BackOnly,
            DoubleFaceMode::BothSides,
        ];

        for global_mode in test_global_modes {
            let decklist_inputs = [
                "kabira takedown", // Should match front face of DFC → use global setting
                "kabira plateau",  // Should match back face of DFC → always BackOnly
                "cut // ribbons",  // Should match full split card name → use global setting
                "cut",             // Should match first part of split card → use global setting
                "ribbons",         // Should match second part of split card → always BackOnly
            ];

            let mut entries = Vec::new();

            for (i, input) in decklist_inputs.iter().enumerate() {
                // Simulate what the new parse_and_resolve_decklist does
                let mut entry = DecklistEntry {
                    multiple: 1,
                    name: input.to_string(),
                    set: None,
                    lang: None,
                    face_mode: DoubleFaceMode::BothSides, // Default before resolution
                    source_line_number: Some(i),
                };

                // Apply the same logic as in the updated parse_and_resolve_decklist
                if let Some(lookup_result) = lookup.find(input) {
                    entry.name = lookup_result.name;
                    // Apply face mode resolution logic (matches MagicHawk logic)
                    entry.face_mode = match lookup_result.hit {
                        NameMatchMode::Part(1) => DoubleFaceMode::BackOnly, // Back face: always back only
                        _ => global_mode.clone(), // Front face or full name: use global setting
                    };
                } else {
                    entry.face_mode = global_mode.clone(); // No match: use global setting
                }

                entries.push(entry);
            }

            assert_eq!(entries.len(), 5, "Should process 5 entries");

            // Verify the face mode resolution for each entry
            let kabira_takedown = &entries[0];
            assert_eq!(
                kabira_takedown.name.to_lowercase(),
                "kabira takedown // kabira plateau"
            );
            assert_eq!(
                kabira_takedown.face_mode, global_mode,
                "Front face input should use global setting: {:?}",
                global_mode
            );

            let kabira_plateau = &entries[1];
            assert_eq!(
                kabira_plateau.name.to_lowercase(),
                "kabira takedown // kabira plateau"
            );
            assert_eq!(
                kabira_plateau.face_mode,
                DoubleFaceMode::BackOnly,
                "Back face input should always use BackOnly regardless of global setting"
            );

            let cut_ribbons_full = &entries[2];
            assert_eq!(cut_ribbons_full.name.to_lowercase(), "cut // ribbons");
            assert_eq!(
                cut_ribbons_full.face_mode, global_mode,
                "Full split card name should use global setting: {:?}",
                global_mode
            );

            let cut_front = &entries[3];
            assert_eq!(cut_front.name.to_lowercase(), "cut // ribbons");
            assert_eq!(
                cut_front.face_mode, global_mode,
                "First part of split card should use global setting: {:?}",
                global_mode
            );

            let ribbons_back = &entries[4];
            assert_eq!(ribbons_back.name.to_lowercase(), "cut // ribbons");
            assert_eq!(
                ribbons_back.face_mode,
                DoubleFaceMode::BackOnly,
                "Second part input should always use BackOnly regardless of global setting"
            );
        }
    }
}
