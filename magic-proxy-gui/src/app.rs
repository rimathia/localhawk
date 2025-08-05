use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_editor};
use iced::{Element, Length, Task};
use magic_proxy_core::{
    DecklistEntry, DoubleFaceMode, NameMatchMode, PdfOptions, ProxyGenerator,
    force_update_card_lookup, get_card_name_cache_info,
};
use rfd::AsyncFileDialog;

#[derive(Debug, Clone)]
pub enum Message {
    DecklistAction(text_editor::Action),
    ParseDecklist,
    DecklistParsed(Vec<DecklistEntry>),
    ClearDecklist,
    GeneratePdf,
    PdfGenerated(Result<Vec<u8>, String>),
    SavePdf,
    FileSaved(Option<String>),
    ForceUpdateCardNames,
    CardNamesUpdated(Result<String, String>),
    DoubleFaceModeChanged(DoubleFaceMode),
}

pub struct AppState {
    display_text: String,
    decklist_content: text_editor::Content,
    parsed_cards: Vec<DecklistEntry>,
    is_parsing: bool,
    error_message: Option<String>,
    is_generating_pdf: bool,
    generated_pdf: Option<Vec<u8>>,
    is_updating_card_names: bool,
    double_face_mode: DoubleFaceMode,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            display_text: "Welcome to Magic Card Proxy Generator!\nParsing includes fuzzy matching, set/language awareness, and card name resolution.".to_string(),
            decklist_content: text_editor::Content::with_text(
                "2 Black Lotus [VMA]\n1 Counterspell [7ED]\n1 Memory Lapse [ja]\n1 kabira takedown\n1 kabira plateau\n1 cut // ribbons (pakh)",
            ),
            parsed_cards: Vec::new(),
            is_parsing: false,
            error_message: None,
            is_generating_pdf: false,
            generated_pdf: None,
            is_updating_card_names: false,
            double_face_mode: DoubleFaceMode::BothSides,
        }
    }
}

pub fn initialize() -> (AppState, Task<Message>) {
    (AppState::new(), Task::none())
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::DecklistAction(action) => {
            state.decklist_content.perform(action);
        }
        Message::ParseDecklist => {
            let decklist_text = state.decklist_content.text();
            if decklist_text.trim().is_empty() {
                state.error_message = Some("Please enter a decklist first!".to_string());
                return Task::none();
            }

            state.is_parsing = true;
            state.error_message = None;

            // Parse and resolve decklist with global caches (fuzzy matching, set/language awareness)
            return Task::perform(
                async move {
                    match ProxyGenerator::parse_and_resolve_decklist(&decklist_text).await {
                        Ok(cards) => cards,
                        Err(e) => {
                            log::error!("Failed to parse decklist: {}", e);
                            Vec::new() // Return empty list on error
                        }
                    }
                },
                Message::DecklistParsed,
            );
        }
        Message::DecklistParsed(cards) => {
            state.is_parsing = false;
            log::debug!("GUI received parsed cards: {}", cards.len());
            for card in &cards {
                log::debug!(
                    "  GUI card: {}x '{}' [set: {:?}, lang: {:?}, preferred_face: {:?}]",
                    card.multiple,
                    card.name,
                    card.set,
                    card.lang,
                    card.preferred_face
                );
            }
            state.parsed_cards = cards;
            state.error_message = None;
            state.display_text = format!("Parsed {} cards successfully!", state.parsed_cards.len());
        }
        Message::ClearDecklist => {
            state.decklist_content = text_editor::Content::new();
            state.parsed_cards.clear();
            state.error_message = None;
            state.display_text = "Decklist cleared!".to_string();
        }
        Message::GeneratePdf => {
            if state.parsed_cards.is_empty() {
                state.error_message = Some("Please parse a decklist first!".to_string());
                return Task::none();
            }

            state.is_generating_pdf = true;
            state.error_message = None;
            state.generated_pdf = None;

            let cards = state.parsed_cards.clone();
            let double_face_mode = state.double_face_mode.clone();
            return Task::perform(
                async move {
                    // Build card list for PDF generation
                    let mut card_list = Vec::new();

                    for entry in cards {
                        log::debug!("Searching for card: '{}'", entry.name);
                        match ProxyGenerator::search_card(&entry.name).await {
                            Ok(search_result) => {
                                log::debug!(
                                    "Found {} printings for '{}':",
                                    search_result.cards.len(),
                                    entry.name
                                );
                                for (i, card) in search_result.cards.iter().enumerate() {
                                    log::debug!(
                                        "  [{}] '{}' ({}) [{}]",
                                        i,
                                        card.name,
                                        card.set.to_uppercase(),
                                        card.language
                                    );
                                }
                                if let Some(card) = search_result.cards.into_iter().find(|c| {
                                    // First check if the card name matches what we're looking for
                                    let name_matches =
                                        c.name.to_lowercase() == entry.name.to_lowercase();

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
                                }) {
                                    log::debug!(
                                        "Selected card: '{}' ({}) [{}]",
                                        card.name,
                                        card.set.to_uppercase(),
                                        card.language
                                    );
                                    // Use per-card face preference if available
                                    log::debug!(
                                        "Entry details: '{}' [set: {:?}, lang: {:?}, preferred_face: {:?}]",
                                        entry.name,
                                        entry.set,
                                        entry.lang,
                                        entry.preferred_face
                                    );
                                    let face_mode = match entry.preferred_face {
                                        Some(NameMatchMode::Part(0)) => {
                                            log::debug!(
                                                "Card '{}' matched front face (Part 0), using global setting: {:?}",
                                                entry.name,
                                                double_face_mode
                                            );
                                            double_face_mode.clone() // Front face: use global setting
                                        }
                                        Some(NameMatchMode::Part(1)) => {
                                            log::debug!(
                                                "Card '{}' matched back face (Part 1), always using BackOnly",
                                                entry.name
                                            );
                                            DoubleFaceMode::BackOnly // Back face: always back only
                                        }
                                        _ => {
                                            log::debug!(
                                                "Card '{}' matched full name or no preference (mode: {:?}), using global setting: {:?}",
                                                entry.name,
                                                entry.preferred_face,
                                                double_face_mode
                                            );
                                            double_face_mode.clone() // Full match or no preference: use global setting
                                        }
                                    };

                                    card_list.push((card, entry.multiple as u32, face_mode));
                                }
                            }
                            Err(e) => {
                                log::debug!("Failed to search for card '{}': {:?}", entry.name, e);
                                // Skip cards that can't be found
                                continue;
                            }
                        }
                    }

                    // Generate PDF using the selected double face mode
                    let pdf_options = PdfOptions {
                        double_face_mode: double_face_mode,
                        ..PdfOptions::default()
                    };
                    match ProxyGenerator::generate_pdf_from_cards_with_face_modes(
                        &card_list,
                        pdf_options,
                        |_current, _total| {
                            // No progress reporting for now
                        },
                    )
                    .await
                    {
                        Ok(pdf_data) => Ok(pdf_data),
                        Err(e) => Err(format!("PDF generation failed: {}", e)),
                    }
                },
                Message::PdfGenerated,
            );
        }
        Message::PdfGenerated(result) => {
            state.is_generating_pdf = false;

            match result {
                Ok(pdf_data) => {
                    state.generated_pdf = Some(pdf_data.clone());
                    state.display_text =
                        format!("PDF generated successfully! {} bytes", pdf_data.len());
                }
                Err(error) => {
                    state.error_message = Some(error);
                    state.display_text = "PDF generation failed!".to_string();
                }
            }
        }
        Message::SavePdf => {
            if state.generated_pdf.is_none() {
                state.error_message = Some("No PDF to save! Generate a PDF first.".to_string());
                return Task::none();
            }

            return Task::perform(
                async {
                    match AsyncFileDialog::new()
                        .set_file_name("proxy_sheet.pdf")
                        .add_filter("PDF Files", &["pdf"])
                        .save_file()
                        .await
                    {
                        Some(handle) => Some(handle.path().to_string_lossy().to_string()),
                        None => None,
                    }
                },
                Message::FileSaved,
            );
        }
        Message::FileSaved(file_path) => {
            if let Some(path) = file_path {
                if let Some(pdf_data) = &state.generated_pdf {
                    match std::fs::write(&path, pdf_data) {
                        Ok(_) => {
                            state.display_text = format!("PDF saved successfully to: {}", path);
                            state.error_message = None;
                        }
                        Err(e) => {
                            state.error_message = Some(format!("Failed to save PDF: {}", e));
                        }
                    }
                } else {
                    state.error_message = Some("No PDF data to save!".to_string());
                }
            } else {
                // User cancelled the dialog
                state.display_text = "Save cancelled.".to_string();
            }
        }
        Message::ForceUpdateCardNames => {
            state.is_updating_card_names = true;
            state.error_message = None;

            return Task::perform(
                async {
                    match force_update_card_lookup().await {
                        Ok(_) => {
                            // Get cache info after update
                            if let Some((timestamp, count)) = get_card_name_cache_info() {
                                Ok(format!(
                                    "Updated {} card names at {}",
                                    count,
                                    timestamp
                                        .format(&time::format_description::well_known::Rfc3339)
                                        .unwrap_or_else(|_| "unknown time".to_string())
                                ))
                            } else {
                                Ok("Updated card names successfully".to_string())
                            }
                        }
                        Err(e) => Err(format!("Failed to update card names: {}", e)),
                    }
                },
                Message::CardNamesUpdated,
            );
        }
        Message::CardNamesUpdated(result) => {
            state.is_updating_card_names = false;

            match result {
                Ok(_) => {
                    state.display_text = "Card names updated successfully!".to_string();
                    state.error_message = None;
                }
                Err(error) => {
                    state.error_message = Some(error);
                    state.display_text = "Card name update failed!".to_string();
                }
            }
        }
        Message::DoubleFaceModeChanged(mode) => {
            state.double_face_mode = mode;
        }
    }
    Task::none()
}

pub fn view(state: &AppState) -> Element<Message> {
    let decklist_section = column![
        text("Decklist Parser:").size(18),
        text("Paste your decklist below (supports various formats):").size(14),
        text_editor(&state.decklist_content)
            .on_action(Message::DecklistAction)
            .height(Length::Fixed(150.0)),
        row![
            button(if state.is_parsing {
                "Parsing..."
            } else {
                "Parse Decklist"
            })
            .on_press_maybe(if state.is_parsing {
                None
            } else {
                Some(Message::ParseDecklist)
            })
            .padding(10),
            button("Clear Decklist")
                .on_press(Message::ClearDecklist)
                .padding(10),
        ]
        .spacing(10),
    ]
    .spacing(10);

    let parsed_cards_section = if !state.parsed_cards.is_empty() {
        let cards_list = scrollable(
            column(
                state
                    .parsed_cards
                    .iter()
                    .map(|card| {
                        let set_info = if let Some(set) = &card.set {
                            format!(" • Set: {}", set.to_uppercase())
                        } else {
                            String::new()
                        };
                        let lang_info = if let Some(lang) = &card.lang {
                            format!(" • Lang: {}", lang.to_uppercase())
                        } else {
                            String::new()
                        };

                        text(format!(
                            "{}x {}{}{}",
                            card.multiple, card.name, set_info, lang_info
                        ))
                        .size(14)
                        .into()
                    })
                    .collect::<Vec<Element<Message>>>(),
            )
            .spacing(2),
        )
        .height(Length::Fixed(200.0));

        column![
            row![
                text(format!("Parsed Cards ({}):", state.parsed_cards.len())).size(16),
                button(if state.is_generating_pdf {
                    "Generating PDF..."
                } else {
                    "Generate PDF"
                })
                .on_press_maybe(if state.is_generating_pdf {
                    None
                } else {
                    Some(Message::GeneratePdf)
                })
                .padding(10),
            ]
            .spacing(10),
            row![
                text("Double-faced cards:").size(14),
                pick_list(
                    DoubleFaceMode::all(),
                    Some(state.double_face_mode.clone()),
                    Message::DoubleFaceModeChanged,
                )
                .width(Length::Fixed(150.0)),
            ]
            .spacing(10),
            cards_list,
        ]
        .spacing(10)
    } else {
        column![]
    };

    let pdf_status_section = if state.is_generating_pdf {
        column![text("Generating PDF...").size(16),].spacing(5)
    } else if let Some(pdf_data) = &state.generated_pdf {
        column![
            row![
                text("PDF Ready!").size(16),
                button("Save PDF").on_press(Message::SavePdf).padding(10),
            ]
            .spacing(10),
            text(format!("Size: {} KB", pdf_data.len() / 1024)).size(14),
        ]
        .spacing(5)
    } else {
        column![]
    };

    let error_section = if let Some(error) = &state.error_message {
        column![text("Error:").size(16), text(error).size(14),].spacing(5)
    } else {
        column![]
    };

    let display_section = column![text(&state.display_text).size(16),].spacing(10);

    let update_section = column![
        row![
            text("Card Name Database:").size(16),
            button(if state.is_updating_card_names {
                "Updating..."
            } else {
                "Update Card Names"
            })
            .on_press_maybe(if state.is_updating_card_names {
                None
            } else {
                Some(Message::ForceUpdateCardNames)
            })
            .padding(10),
        ]
        .spacing(10),
        text(
            get_card_name_cache_info()
                .map(|(timestamp, count)| {
                    format!(
                        "Cache: {} cards, last updated: {}",
                        count,
                        timestamp
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap_or_else(|_| "Unknown".to_string())
                    )
                })
                .unwrap_or_else(|| "No cache found".to_string())
        )
        .size(12),
    ]
    .spacing(5);

    let content = column![
        decklist_section,
        parsed_cards_section,
        pdf_status_section,
        update_section,
        error_section,
        display_section,
    ]
    .spacing(20)
    .padding(20);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
