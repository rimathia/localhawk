use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use crate::{DecklistEntry, DoubleFaceMode, ProxyError};
use crate::globals::{get_or_fetch_search_results, get_or_fetch_image_bytes};

#[derive(Debug, Clone)]
pub struct BackgroundLoadProgress {
    pub phase: LoadingPhase,
    pub current_entry: usize,
    pub total_entries: usize,
    pub selected_loaded: usize,
    pub alternatives_loaded: usize,
    pub total_alternatives: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadingPhase {
    Selected,     // Loading selected printings (based on set/lang hints)
    Alternatives, // Loading alternative printings
    Completed,    // All done
}

pub struct BackgroundLoadHandle {
    handle: JoinHandle<Result<(), ProxyError>>,
    progress_rx: tokio::sync::mpsc::UnboundedReceiver<BackgroundLoadProgress>,
    cancel_token: CancellationToken,
}

impl BackgroundLoadHandle {
    /// Get latest progress (non-blocking)
    pub fn try_get_progress(&mut self) -> Option<BackgroundLoadProgress> {
        // Drain all available progress messages and return the latest one
        let mut latest_progress = None;
        while let Ok(progress) = self.progress_rx.try_recv() {
            latest_progress = Some(progress);
        }
        latest_progress
    }
    
    /// Cancel background loading
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
    
    /// Wait for completion
    pub async fn wait_for_completion(self) -> Result<(), ProxyError> {
        self.handle.await
            .map_err(|e| ProxyError::Cache(format!("Task join error: {}", e)))?
    }
    
    /// Check if finished (non-blocking)
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }
}

/// Start background image loading for resolved decklist entries
pub fn start_background_image_loading(entries: Vec<DecklistEntry>) -> BackgroundLoadHandle {
    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel_token = CancellationToken::new();
    let cancel_clone = cancel_token.clone();
    
    log::debug!("Starting background image loading for {} entries", entries.len());
    
    let handle = tokio::spawn(async move {
        load_background_images_impl(entries, progress_tx, cancel_clone).await
    });
    
    BackgroundLoadHandle {
        handle,
        progress_rx,
        cancel_token,
    }
}

async fn load_background_images_impl(
    entries: Vec<DecklistEntry>,
    progress_tx: UnboundedSender<BackgroundLoadProgress>,
    cancel_token: CancellationToken,
) -> Result<(), ProxyError> {
    let mut selected_loaded = 0;
    let mut alternatives_loaded = 0;
    let mut total_alternatives = 0;
    let mut errors = Vec::new();
    
    // Phase 1: Load Selected Printings
    send_progress(&progress_tx, BackgroundLoadProgress {
        phase: LoadingPhase::Selected,
        current_entry: 0,
        total_entries: entries.len(),
        selected_loaded: 0,
        alternatives_loaded: 0,
        total_alternatives: 0,
        errors: errors.clone(),
    });
    
    log::debug!("Starting SELECTED phase - loading {} entries", entries.len());
    
    for (entry_idx, entry) in entries.iter().enumerate() {
        if cancel_token.is_cancelled() {
            log::debug!("Background loading cancelled during SELECTED phase at entry {}", entry_idx);
            return Ok(());
        }
        
        log::debug!("SELECTED Phase - Loading entry {}/{}: '{}' [set: {:?}, lang: {:?}, face_mode: {:?}]", 
            entry_idx + 1, entries.len(), entry.name, entry.set, entry.lang, entry.face_mode);
        
        // Search for card (uses search cache)
        match get_or_fetch_search_results(&entry.name).await {
            Ok(search_result) => {
                // Select printing based on entry's set/lang hints
                if let Some(selected_index) = select_card_from_printings(&search_result.cards, entry) {
                    let selected_card = &search_result.cards[selected_index];
                    
                    log::debug!("  Selected printing {}/{}: '{}' ({}) [{}] - caching images for face_mode: {:?}",
                        selected_index + 1, search_result.cards.len(),
                        selected_card.name, selected_card.set.to_uppercase(), selected_card.language, entry.face_mode);
                    
                    // Count alternatives (all except selected)
                    let alternatives_for_card = search_result.cards.len().saturating_sub(1);
                    total_alternatives += alternatives_for_card;
                    
                    log::debug!("    {} alternatives available for this card", alternatives_for_card);
                    
                    // Cache images for selected printing (front/back based on face_mode)
                    let urls = get_image_urls_for_face_mode(selected_card, &entry.face_mode);
                    log::debug!("  Will cache {} image(s) for this entry", urls.len());
                    
                    for url in urls {
                        log::debug!("    Caching image: {}", url);
                        if let Err(e) = get_or_fetch_image_bytes(&url).await {
                            let error_msg = format!("Failed to cache {}: {}", url, e);
                            log::warn!("{}", error_msg);
                            errors.push(error_msg);
                        } else {
                            log::debug!("      ✓ Successfully cached image");
                        }
                    }
                    
                    selected_loaded += 1;
                } else {
                    let error_msg = format!("No suitable printing found for '{}'", entry.name);
                    log::warn!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
            Err(e) => {
                let error_msg = format!("Search failed for '{}': {}", entry.name, e);
                log::warn!("{}", error_msg);
                errors.push(error_msg);
            }
        }
        
        // Send progress update
        send_progress(&progress_tx, BackgroundLoadProgress {
            phase: LoadingPhase::Selected,
            current_entry: entry_idx + 1,
            total_entries: entries.len(),
            selected_loaded,
            alternatives_loaded,
            total_alternatives,
            errors: errors.clone(),
        });
    }
    
    log::debug!("SELECTED Phase complete - switching to ALTERNATIVES phase");
    
    // Phase 2: Load Alternative Printings
    send_progress(&progress_tx, BackgroundLoadProgress {
        phase: LoadingPhase::Alternatives,
        current_entry: entries.len(),
        total_entries: entries.len(),
        selected_loaded,
        alternatives_loaded: 0,
        total_alternatives,
        errors: errors.clone(),
    });
    
    log::debug!("Starting ALTERNATIVES phase - loading {} total alternatives", total_alternatives);
    
    for (entry_idx, entry) in entries.iter().enumerate() {
        if cancel_token.is_cancelled() {
            log::debug!("Background loading cancelled during ALTERNATIVES phase at entry {}", entry_idx);
            return Ok(());
        }
        
        if let Ok(search_result) = get_or_fetch_search_results(&entry.name).await {
            // Cache all alternatives (skip selected printing)
            let selected_index = select_card_from_printings(&search_result.cards, entry);
            
            for (card_idx, card) in search_result.cards.iter().enumerate() {
                if Some(card_idx) == selected_index {
                    continue; // Skip selected printing (already cached)
                }
                
                if cancel_token.is_cancelled() {
                    log::debug!("Background loading cancelled during alternative loading");
                    return Ok(());
                }
                
                log::debug!("ALTERNATIVES Phase - Loading alternative printing: '{}' ({}) [{}]",
                    card.name, card.set.to_uppercase(), card.language);
                
                // Cache front image for alternative (most common use case)
                if let Err(e) = get_or_fetch_image_bytes(&card.border_crop).await {
                    let error_msg = format!("Failed to cache alternative {}: {}", card.border_crop, e);
                    log::warn!("{}", error_msg);
                    errors.push(error_msg);
                }
                
                alternatives_loaded += 1;
                
                // Send progress update for every alternative (no throttling to ensure accurate progress)
                log::debug!("Sending alternatives progress: {}/{}", alternatives_loaded, total_alternatives);
                send_progress(&progress_tx, BackgroundLoadProgress {
                    phase: LoadingPhase::Alternatives,
                    current_entry: entries.len(),
                    total_entries: entries.len(),
                    selected_loaded,
                    alternatives_loaded,
                    total_alternatives,
                    errors: errors.clone(),
                });
            }
        }
    }
    
    log::debug!("Background loading completed - {} selected + {} alternatives = {} total images",
        selected_loaded, alternatives_loaded, selected_loaded + alternatives_loaded);
    
    // Final progress
    send_progress(&progress_tx, BackgroundLoadProgress {
        phase: LoadingPhase::Completed,
        current_entry: entries.len(),
        total_entries: entries.len(),
        selected_loaded,
        alternatives_loaded,
        total_alternatives,
        errors,
    });
    
    Ok(())
}

fn send_progress(tx: &UnboundedSender<BackgroundLoadProgress>, progress: BackgroundLoadProgress) {
    if tx.send(progress).is_err() {
        // Receiver dropped, ignore
        log::debug!("Progress receiver dropped, stopping progress updates");
    }
}

/// Select the best card from available printings based on DecklistEntry preferences
/// This mirrors the logic from the GUI's select_card_from_printings function
fn select_card_from_printings(
    available_printings: &[crate::scryfall::models::Card],
    entry: &DecklistEntry,
) -> Option<usize> {
    available_printings.iter().position(|card| {
        // First check if the card name matches what we're looking for
        let name_matches = card.name.to_lowercase() == entry.name.to_lowercase();
        
        // Try to match both set and language if specified
        let set_matches = if let Some(ref entry_set) = entry.set {
            card.set.to_lowercase() == entry_set.to_lowercase()
        } else {
            true // No set preference, any set is fine
        };
        
        let lang_matches = if let Some(ref entry_lang) = entry.lang {
            card.language.to_lowercase() == entry_lang.to_lowercase()
        } else {
            true // No language preference, any language is fine
        };
        
        name_matches && set_matches && lang_matches
    })
}

/// Get image URLs for a card based on face mode
/// This mirrors the logic from the GUI's image URL handling
fn get_image_urls_for_face_mode(
    card: &crate::scryfall::models::Card,
    face_mode: &DoubleFaceMode,
) -> Vec<String> {
    let mut urls = Vec::new();
    
    match face_mode {
        DoubleFaceMode::FrontOnly => {
            urls.push(card.border_crop.clone());
        }
        DoubleFaceMode::BackOnly => {
            if let Some(ref back_url) = card.border_crop_back {
                urls.push(back_url.clone());
            } else {
                // Fallback to front if no back available
                urls.push(card.border_crop.clone());
            }
        }
        DoubleFaceMode::BothSides => {
            urls.push(card.border_crop.clone());
            if let Some(ref back_url) = card.border_crop_back {
                urls.push(back_url.clone());
            }
        }
    }
    
    urls
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scryfall::models::Card;
    
    #[test]
    fn test_select_card_from_printings_with_set_preference() {
        let cards = vec![
            Card {
                name: "Lightning Bolt".to_string(),
                set: "LEA".to_string(),
                language: "en".to_string(),
                border_crop: "url1".to_string(),
                border_crop_back: None,
                meld_result: None,
            },
            Card {
                name: "Lightning Bolt".to_string(),
                set: "VMA".to_string(),
                language: "en".to_string(),
                border_crop: "url2".to_string(),
                border_crop_back: None,
                meld_result: None,
            },
        ];
        
        let entry = DecklistEntry {
            multiple: 1,
            name: "Lightning Bolt".to_string(),
            set: Some("VMA".to_string()),
            lang: None,
            face_mode: DoubleFaceMode::BothSides,
            source_line_number: None,
        };
        
        let result = select_card_from_printings(&cards, &entry);
        assert_eq!(result, Some(1)); // Should select VMA printing
    }
    
    #[test]
    fn test_get_image_urls_for_face_mode() {
        let card = Card {
            name: "Test Card".to_string(),
            set: "TST".to_string(),
            language: "en".to_string(),
            border_crop: "front_url".to_string(),
            border_crop_back: Some("back_url".to_string()),
            meld_result: None,
        };
        
        // Test FrontOnly
        let urls = get_image_urls_for_face_mode(&card, &DoubleFaceMode::FrontOnly);
        assert_eq!(urls, vec!["front_url"]);
        
        // Test BackOnly
        let urls = get_image_urls_for_face_mode(&card, &DoubleFaceMode::BackOnly);
        assert_eq!(urls, vec!["back_url"]);
        
        // Test BothSides
        let urls = get_image_urls_for_face_mode(&card, &DoubleFaceMode::BothSides);
        assert_eq!(urls, vec!["front_url", "back_url"]);
    }
}