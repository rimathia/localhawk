use super::{client::ScryfallClient, models::*};
use crate::error::ProxyError;
use log::info;

const SCRYFALL_CARD_NAMES: &str = "https://api.scryfall.com/catalog/card-names";
const SCRYFALL_SETS: &str = "https://api.scryfall.com/sets";

impl ScryfallClient {
    pub async fn get_card_names(&self) -> Result<ScryfallCardNames, ProxyError> {
        let response = self.call(SCRYFALL_CARD_NAMES).await?;
        let mut card_names: ScryfallCardNames = response.json().await?;

        card_names.date = Some(time::OffsetDateTime::now_utc());
        for name in card_names.names.iter_mut() {
            *name = name.to_lowercase();
        }

        Ok(card_names)
    }

    pub async fn get_set_codes(&self) -> Result<ScryfallSetCodes, ProxyError> {
        let response = self.call(SCRYFALL_SETS).await?;
        let sets_response: ScryfallSetsResponse = response.json().await?;

        let codes = sets_response
            .data
            .into_iter()
            .map(|set| set.code.to_lowercase())
            .collect();

        Ok(ScryfallSetCodes {
            date: Some(time::OffsetDateTime::now_utc()),
            codes,
        })
    }

    async fn get_exact_name_matches(&self, name: &str) -> Result<CardSearchResult, ProxyError> {
        let encoded_name = encode_card_name(name);
        let uri = format!(
            "https://api.scryfall.com/cards/search?q=name:\"{}\"&unique=prints",
            encoded_name
        );

        log::debug!("Searching Scryfall with URI: {}", uri);
        let response = self.call(&uri).await?;

        match response.json::<ScryfallSearchAnswer>().await {
            Ok(answer) => {
                let mut cards = Vec::new();
                let search_name_lower = name.to_lowercase();

                for card_data in answer.data {
                    match Card::from_scryfall_object(&card_data) {
                        Ok(card) => {
                            // Filter results to only include cards that exactly match our search name
                            let card_name_lower = card.name.to_lowercase();
                            if card_name_lower == search_name_lower {
                                log::debug!("Adding exact match: '{}' ({})", card.name, card.set);
                                cards.push(card);
                            } else {
                                log::debug!(
                                    "Skipping non-exact match: '{}' != '{}'",
                                    card.name,
                                    name
                                );
                            }
                        }
                        Err(e) => {
                            info!("Skipping invalid card: {}", e);
                            continue;
                        }
                    }
                }

                log::debug!(
                    "Filtered {} cards from {} total results",
                    cards.len(),
                    answer.total_cards
                );
                Ok(CardSearchResult {
                    total_found: cards.len(),
                    cards: cards,
                })
            }
            Err(e) => {
                info!("Error deserializing Scryfall search: {}", e);
                Err(ProxyError::Network(e))
            }
        }
    }

    pub async fn search_card(&self, name: &str) -> Result<CardSearchResult, ProxyError> {
        let name_matches = self.get_exact_name_matches(name).await?;
        let mut cards = name_matches.cards;

        for card in &mut cards {
            if let Some(super::models::BackSide::ContributesToMeld {
                meld_result_name,
                meld_result_image_url,
                ..
            }) = &mut card.back_side
            {
                if meld_result_image_url.is_empty() {
                    log::debug!(
                        "Resolving meld result '{}' for card '{}'",
                        meld_result_name,
                        card.name
                    );

                    // Search for the meld result card (without recursively resolving meld results)
                    let meld_search_result = self.get_exact_name_matches(&meld_result_name).await?;

                    if meld_search_result.cards.is_empty() {
                        return Err(ProxyError::InvalidCard(format!(
                            "Meld result '{}' not found",
                            meld_result_name
                        )));
                    }

                    // Debug: Log all search results for the meld result
                    log::debug!(
                        "Meld search for '{}' returned {} cards:",
                        meld_result_name,
                        meld_search_result.cards.len()
                    );
                    for (i, result_card) in meld_search_result.cards.iter().enumerate() {
                        log::debug!(
                            "  [{}] '{}' (set: {}) - URL: {}",
                            i,
                            result_card.name,
                            result_card.set,
                            result_card.border_crop
                        );
                    }

                    // Find a meld result card that matches the same set as the original card, or use the first one
                    let meld_card = meld_search_result
                        .cards
                        .iter()
                        .find(|meld_card| meld_card.set == card.set)
                        .or_else(|| meld_search_result.cards.first())
                        .ok_or_else(|| {
                            ProxyError::InvalidCard("No meld result card available".to_string())
                        })?;

                    log::debug!(
                        "Found meld result '{}' (set: {}) for card '{}' (set: {})",
                        meld_card.name,
                        meld_card.set,
                        card.name,
                        card.set
                    );
                    *meld_result_image_url = meld_card.border_crop.clone();
                }
            }
        }

        Ok(CardSearchResult {
            cards,
            total_found: name_matches.total_found,
        })

        //        log::debug!("Searching Scryfall with URI: {}", uri);
        //        let response = self.call(&uri).await?;
        //
        //        match response.json::<ScryfallSearchAnswer>().await {
        //            Ok(answer) => {
        //                let mut cards = Vec::new();
        //                let search_name_lower = name.to_lowercase();
        //
        //                for card_data in answer.data {
        //                    match Card::from_scryfall_object(&card_data) {
        //                        Ok(card) => {
        //                            // Filter results to only include cards that exactly match our search name
        //                            let card_name_lower = card.name.to_lowercase();
        //                            if card_name_lower == search_name_lower {
        //                                log::debug!("Adding exact match: '{}' ({})", card.name, card.set);
        //                                cards.push(card);
        //                            } else {
        //                                log::debug!(
        //                                    "Skipping non-exact match: '{}' != '{}'",
        //                                    card.name,
        //                                    name
        //                                );
        //                            }
        //                        }
        //                        Err(e) => {
        //                            info!("Skipping invalid card: {}", e);
        //                            continue;
        //                        }
        //                    }
        //                }
        //
        //                log::debug!(
        //                    "Filtered {} cards from {} total results",
        //                    cards.len(),
        //                    answer.total_cards
        //                );
        //
        //                // Resolve meld results for any meld cards (only if resolve_melds is true)
        //                let mut resolved_cards = Vec::new();
        //                for mut card in cards {
        //                    if resolve_melds && card.is_meld_card() {
        //                        let needs_resolution =
        //                            if let Some(super::models::BackSide::ContributesToMeld {
        //                                meld_result_image_url,
        //                                ..
        //                            }) = &card.back_side
        //                            {
        //                                meld_result_image_url.is_empty()
        //                            } else {
        //                                false
        //                            };
        //
        //                        if needs_resolution {
        //                            if let Some(super::models::BackSide::ContributesToMeld {
        //                                meld_result_name,
        //                                ..
        //                            }) = &card.back_side
        //                            {
        //                                let meld_name = meld_result_name.clone();
        //                                log::debug!(
        //                                    "Resolving meld result '{}' for card '{}'",
        //                                    meld_name,
        //                                    card.name
        //                                );
        //                                match self.resolve_meld_result(&mut card, &meld_name).await {
        //                                    Ok(_) => log::debug!(
        //                                        "Successfully resolved meld result for '{}'",
        //                                        card.name
        //                                    ),
        //                                    Err(e) => log::warn!(
        //                                        "Failed to resolve meld result for '{}': {}",
        //                                        card.name,
        //                                        e
        //                                    ),
        //                                }
        //                            }
        //                        }
        //                    }
        //                    resolved_cards.push(card);
        //                }
        //
        //                Ok(CardSearchResult {
        //                    total_found: resolved_cards.len(),
        //                    cards: resolved_cards,
        //                })
        //            }
        //            Err(e) => {
        //                info!("Error deserializing Scryfall search: {}", e);
        //                Err(ProxyError::Network(e))
        //            }
        //        }
    }

    // /// Resolve meld result by searching for the meld result card and setting its image as border_crop_back
    // /// This follows the MagicHawk approach to handling meld cards
    // fn resolve_meld_result<'a>(
    //     &'a self,
    //     card: &'a mut Card,
    //     meld_result_name: &'a str,
    // ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ProxyError>> + Send + 'a>>
    // {
    //     Box::pin(async move {
    //         log::debug!("Searching for meld result: '{}'", meld_result_name);

    //         // Search for the meld result card (without recursively resolving meld results)
    //         let meld_search_result = self.search_card_internal(meld_result_name, false).await?;

    //         if meld_search_result.cards.is_empty() {
    //             return Err(ProxyError::InvalidCard(format!(
    //                 "Meld result '{}' not found",
    //                 meld_result_name
    //             )));
    //         }

    //         // Debug: Log all search results for the meld result
    //         log::debug!(
    //             "Meld search for '{}' returned {} cards:",
    //             meld_result_name,
    //             meld_search_result.cards.len()
    //         );
    //         for (i, result_card) in meld_search_result.cards.iter().enumerate() {
    //             log::debug!(
    //                 "  [{}] '{}' (set: {}) - URL: {}",
    //                 i,
    //                 result_card.name,
    //                 result_card.set,
    //                 result_card.border_crop
    //             );
    //         }

    //         // Find a meld result card that matches the same set as the original card, or use the first one
    //         let meld_card = meld_search_result
    //             .cards
    //             .iter()
    //             .find(|meld_card| meld_card.set == card.set)
    //             .or_else(|| meld_search_result.cards.first())
    //             .ok_or_else(|| {
    //                 ProxyError::InvalidCard("No meld result card available".to_string())
    //             })?;

    //         log::debug!(
    //             "Found meld result '{}' (set: {}) for card '{}' (set: {})",
    //             meld_card.name,
    //             meld_card.set,
    //             card.name,
    //             card.set
    //         );

    //         // Update the meld result image URL in the ContributesToMeld structure
    //         if let Some(super::models::BackSide::ContributesToMeld {
    //             meld_result_image_url,
    //             ..
    //         }) = &mut card.back_side
    //         {
    //             *meld_result_image_url = meld_card.border_crop.clone();
    //         } else {
    //             return Err(ProxyError::InvalidCard(
    //                 "Card is not a meld card".to_string(),
    //             ));
    //         }

    //         Ok(())
    //     })
    // }
}

fn encode_card_name(name: &str) -> String {
    // Proper URL encoding for card names
    // Handle spaces, slashes, and other special characters
    name.chars()
        .map(|c| match c {
            ' ' => "+".to_string(),
            '/' => "%2F".to_string(),
            '"' => "%22".to_string(),
            '\'' => "%27".to_string(),
            '&' => "%26".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c.to_string(),
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}
