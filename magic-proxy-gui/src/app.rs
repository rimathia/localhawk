use iced::widget::{button, column, container, row, scrollable, text, text_editor};
use iced::{Element, Length, Task};
use magic_proxy_core::{DecklistEntry, parse_decklist, get_minimal_scryfall_languages, ProxyGenerator, ScryfallClient, ApiCall, ApiCallType, PdfOptions, force_update_card_lookup, get_card_name_cache_info};
use rfd::AsyncFileDialog;

#[derive(Debug, Clone)]
pub enum Message {
    DecklistAction(text_editor::Action),
    ParseDecklist,
    ParseDecklistWithFuzzyMatching,
    DecklistParsed(Vec<DecklistEntry>),
    ClearDecklist,
    RefreshApiHistory,
    ClearApiHistory,
    ToggleApiHistoryView,
    GeneratePdf,
    PdfGenerated(Result<Vec<u8>, String>),
    SavePdf,
    FileSaved(Option<String>),
    ForceUpdateCardNames,
    CardNamesUpdated(Result<String, String>),
}

pub struct AppState {
    display_text: String,
    decklist_content: text_editor::Content,
    parsed_cards: Vec<DecklistEntry>,
    is_parsing: bool,
    error_message: Option<String>,
    api_history: Vec<ApiCall>,
    show_api_history: bool,
    is_generating_pdf: bool,
    generated_pdf: Option<Vec<u8>>,
    is_updating_card_names: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            display_text: "Welcome to Magic Card Proxy Generator!\nOffline parsing available. Use 'Parse with Fuzzy Matching' for network-enabled card name resolution.".to_string(),
            decklist_content: text_editor::Content::with_text(
                "4 Lightning Bolt\n1 Black Lotus [VMA]\n2 Counterspell [7ED]\n3 Giant Growth\n1 Memory Lapse [ja]",
            ),
            parsed_cards: Vec::new(),
            is_parsing: false,
            error_message: None,
            api_history: Vec::new(),
            show_api_history: false,
            is_generating_pdf: false,
            generated_pdf: None,
            is_updating_card_names: false,
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

            return Task::perform(
                async move {
                    // Parse the decklist without network calls (no fuzzy matching)
                    let languages = get_minimal_scryfall_languages();
                    let parsed_lines = parse_decklist(&decklist_text, &languages);
                    
                    let mut resolved_entries = Vec::new();
                    for line in parsed_lines {
                        if let Some(entry) = line.as_entry() {
                            resolved_entries.push(entry);
                        }
                    }
                    
                    resolved_entries
                },
                Message::DecklistParsed,
            );
        }
        Message::ParseDecklistWithFuzzyMatching => {
            let decklist_text = state.decklist_content.text();
            if decklist_text.trim().is_empty() {
                state.error_message = Some("Please enter a decklist first!".to_string());
                return Task::none();
            }

            state.is_parsing = true;
            state.error_message = None;

            return Task::perform(
                async move {
                    // Use the new static method - no expensive ProxyGenerator initialization!
                    match ProxyGenerator::parse_and_resolve_decklist(&decklist_text).await {
                        Ok(cards) => cards,
                        Err(e) => {
                            log::error!("Failed to parse decklist with fuzzy matching: {}", e);
                            // Fall back to offline parsing
                            let languages = get_minimal_scryfall_languages();
                            let parsed_lines = parse_decklist(&decklist_text, &languages);
                            
                            let mut resolved_entries = Vec::new();
                            for line in parsed_lines {
                                if let Some(entry) = line.as_entry() {
                                    resolved_entries.push(entry);
                                }
                            }
                            resolved_entries
                        }
                    }
                },
                Message::DecklistParsed,
            );
        }
        Message::DecklistParsed(cards) => {
            state.is_parsing = false;
            state.parsed_cards = cards;
            state.error_message = None;
            state.display_text = format!("Parsed {} cards successfully!", state.parsed_cards.len());
            
            // Auto-refresh API history if it's visible
            if state.show_api_history {
                state.api_history = ScryfallClient::get_api_call_history();
            }
        }
        Message::ClearDecklist => {
            state.decklist_content = text_editor::Content::new();
            state.parsed_cards.clear();
            state.error_message = None;
            state.display_text = "Decklist cleared!".to_string();
        }
        Message::RefreshApiHistory => {
            state.api_history = ScryfallClient::get_api_call_history();
        }
        Message::ClearApiHistory => {
            ScryfallClient::clear_api_call_history();
            state.api_history.clear();
        }
        Message::ToggleApiHistoryView => {
            state.show_api_history = !state.show_api_history;
            if state.show_api_history {
                state.api_history = ScryfallClient::get_api_call_history();
            }
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
            return Task::perform(
                async move {
                    // Build card list for PDF generation
                    let mut card_list = Vec::new();
                    
                    for entry in cards {
                        match ProxyGenerator::search_card(&entry.name).await {
                            Ok(search_result) => {
                                if let Some(card) = search_result.cards.into_iter().find(|c| {
                                    // Try to match set if specified
                                    if let Some(ref entry_set) = entry.set {
                                        c.set.to_lowercase() == entry_set.to_lowercase()
                                    } else {
                                        true // Take first result if no set specified
                                    }
                                }) {
                                    card_list.push((card, entry.multiple as u32));
                                }
                            }
                            Err(_) => {
                                // Skip cards that can't be found
                                continue;
                            }
                        }
                    }

                    // Generate PDF using the new static method
                    match ProxyGenerator::generate_pdf_from_cards(&card_list, PdfOptions::default(), |_current, _total| {
                        // No progress reporting for now
                    }).await {
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
                    state.display_text = format!("PDF generated successfully! {} bytes", pdf_data.len());
                    
                    // Auto-refresh API history if visible
                    if state.show_api_history {
                        state.api_history = ScryfallClient::get_api_call_history();
                    }
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
                                Ok(format!("Updated {} card names at {}", count, timestamp.format(&time::format_description::well_known::Rfc3339).unwrap_or_else(|_| "unknown time".to_string())))
                            } else {
                                Ok("Updated card names successfully".to_string())
                            }
                        },
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
                    
                    // Auto-refresh API history if visible
                    if state.show_api_history {
                        state.api_history = ScryfallClient::get_api_call_history();
                    }
                }
                Err(error) => {
                    state.error_message = Some(error);
                    state.display_text = "Card name update failed!".to_string();
                }
            }
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
                "Parse Decklist (Offline)"
            })
            .on_press_maybe(if state.is_parsing {
                None
            } else {
                Some(Message::ParseDecklist)
            })
            .padding(10),
            button(if state.is_parsing {
                "Parsing..."
            } else {
                "Parse with Fuzzy Matching"
            })
            .on_press_maybe(if state.is_parsing {
                None
            } else {
                Some(Message::ParseDecklistWithFuzzyMatching)
            })
            .padding(10),
            button("Clear Decklist")
                .on_press(Message::ClearDecklist)
                .padding(10),
            button(if state.show_api_history {
                "Hide API Debug"
            } else {
                "Show API Debug"
            })
            .on_press(Message::ToggleApiHistoryView)
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
                            format!(" [{}]", set.to_uppercase())
                        } else {
                            String::new()
                        };
                        let lang_info = if let Some(lang) = &card.lang {
                            format!(" ({})", lang.to_uppercase())
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
            cards_list,
        ]
        .spacing(10)
    } else {
        column![]
    };

    let pdf_status_section = if state.is_generating_pdf {
        column![
            text("Generating PDF...").size(16),
        ]
        .spacing(5)
    } else if let Some(pdf_data) = &state.generated_pdf {
        column![
            row![
                text("PDF Ready!").size(16),
                button("Save PDF")
                    .on_press(Message::SavePdf)
                    .padding(10),
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

    let api_history_section = if state.show_api_history {
        let history_list = if !state.api_history.is_empty() {
            scrollable(
                column(
                    state
                        .api_history
                        .iter()
                        .map(|call| {
                            let (status_icon, call_type_icon) = match call.call_type {
                                ApiCallType::NetworkRequest => {
                                    let status = if call.success { "✅" } else { "❌" };
                                    (status, "🌐")
                                },
                                ApiCallType::CacheHit => ("✅", "💾"),
                                ApiCallType::CacheMiss => ("⚠️", "💾"),
                            };
                            
                            let timestamp_str = call.timestamp.format(&time::format_description::well_known::Rfc3339).unwrap_or_else(|_| "Invalid time".to_string());
                            
                            let status_code_str = match call.call_type {
                                ApiCallType::NetworkRequest => format!("[{}]", call.status_code),
                                ApiCallType::CacheHit => "[CACHE HIT]".to_string(),
                                ApiCallType::CacheMiss => "[CACHE MISS]".to_string(),
                            };
                            
                            text(format!(
                                "{} {} {} {} {}",
                                status_icon,
                                call_type_icon,
                                timestamp_str,
                                status_code_str,
                                call.url
                            ))
                            .size(12)
                            .into()
                        })
                        .collect::<Vec<Element<Message>>>(),
                )
                .spacing(2),
            )
            .height(Length::Fixed(200.0))
        } else {
            scrollable(column![text("No API calls recorded yet.").size(12)])
                .height(Length::Fixed(50.0))
        };

        column![
            row![
                text("API Call History:").size(16),
                button("Refresh")
                    .on_press(Message::RefreshApiHistory)
                    .padding(5),
                button("Clear")
                    .on_press(Message::ClearApiHistory)
                    .padding(5),
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
                .padding(5),
            ]
            .spacing(10),
            history_list,
        ]
        .spacing(10)
    } else {
        column![]
    };

    let content = column![
        decklist_section,
        parsed_cards_section,
        pdf_status_section,
        api_history_section,
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
