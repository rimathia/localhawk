pub mod cache;
pub mod card_name_cache;
pub mod decklist;
pub mod error;
pub mod globals;
pub mod lookup;
pub mod pdf;
pub mod scryfall;
pub mod search_results_cache;

pub use cache::ImageCache;
pub use card_name_cache::CardNameCache;
pub use search_results_cache::SearchResultsCache;
pub use decklist::{DecklistEntry, ParsedDecklistLine, parse_decklist, parse_line};
pub use error::ProxyError;
pub use globals::{
    ensure_card_lookup_initialized, find_card_name, force_update_card_lookup,
    get_card_name_cache_info, get_image_cache, get_or_fetch_image, get_or_fetch_search_results,
    get_scryfall_client, initialize_caches,
};
pub use lookup::{CardNameLookup, NameLookupResult, NameMatchMode};
pub use pdf::{PageSize, PdfOptions, generate_pdf};
pub use scryfall::{
    Card, CardSearchResult, ScryfallCardNames, ScryfallClient,
    models::get_minimal_scryfall_languages,
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
        ensure_card_lookup_initialized().await
    }

    /// Force update card names from Scryfall and reinitialize fuzzy matching (now uses global state)
    pub async fn force_update_card_lookup() -> Result<(), ProxyError> {
        force_update_card_lookup().await
    }

    /// Find a card name using fuzzy matching (now uses global state)
    pub fn find_card_name(name: &str) -> Option<NameLookupResult> {
        find_card_name(name)
    }

    /// Parse a decklist and resolve card names using fuzzy matching
    pub async fn parse_and_resolve_decklist(
        decklist_text: &str,
    ) -> Result<Vec<DecklistEntry>, ProxyError> {
        use scryfall::models::get_minimal_scryfall_languages;

        // Ensure global card lookup is initialized
        ensure_card_lookup_initialized().await?;

        let languages = get_minimal_scryfall_languages();
        let parsed_lines = parse_decklist(decklist_text, &languages);

        let mut resolved_entries = Vec::new();
        for line in parsed_lines {
            if let Some(mut entry) = line.as_entry() {
                // Try to resolve the card name using global fuzzy matching
                if let Some(lookup_result) = find_card_name(&entry.name) {
                    entry.name = lookup_result.name;
                }
                resolved_entries.push(entry);
            }
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

                // Get front image using global cache
                let front_image = get_or_fetch_image(&card.border_crop).await?;
                images.push(front_image);

                // Get back image if it exists
                if let Some(back_url) = &card.border_crop_back {
                    let back_image = get_or_fetch_image(back_url).await?;
                    images.push(back_image);
                }

                current_progress += 1;
            }
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

                // Get front image using global cache
                let front_image = get_or_fetch_image(&card.border_crop).await?;
                images.push(front_image);

                // Get back image if it exists
                if let Some(back_url) = &card.border_crop_back {
                    let back_image = get_or_fetch_image(back_url).await?;
                    images.push(back_image);
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
        cache_guard.size()
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
        cache_guard.force_evict(url)
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
            border_crop_back: None,
            meld_result: None,
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
        assert_eq!(get_image_cache().read().unwrap().size(), 0);

        // Test clear cache (should not panic on empty cache)
        ProxyGenerator::clear_cache().unwrap();
        assert_eq!(get_image_cache().read().unwrap().size(), 0);
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
            border_crop_back: None,
            meld_result: None,
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
        assert_eq!(get_image_cache().read().unwrap().size(), 0);
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
}
