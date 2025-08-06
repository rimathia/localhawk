use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_editor};
use iced::{Element, Length, Task};
use magic_proxy_core::{
    Card, DecklistEntry, DoubleFaceMode, NameMatchMode, PdfOptions, ProxyGenerator,
    force_update_card_lookup, get_card_name_cache_info,
};
use rfd::AsyncFileDialog;

/// Individual card position in the grid layout
#[derive(Debug, Clone, PartialEq)]
pub struct GridPosition {
    pub page: usize,                    // Which page this position is on
    pub position_in_page: usize,        // 0-8 position within 3x3 grid
    pub entry_index: usize,             // Back-reference to parent entry
    pub copy_number: usize,             // 1st, 2nd, 3rd, 4th copy of entry
}

/// Represents one decklist entry with all its printings and positions
#[derive(Debug, Clone)]
pub struct PreviewEntry {
    pub decklist_entry: DecklistEntry,     // Original "4x Lightning Bolt [LEA]"
    pub available_printings: Vec<Card>,    // All printings found from search
    pub selected_printing: Option<usize>,  // Index into available_printings
    pub grid_positions: Vec<GridPosition>, // Where this entry's cards appear
}

impl PreviewEntry {
    /// Get the currently selected card or the first available printing
    pub fn get_selected_card(&self) -> Option<&Card> {
        match self.selected_printing {
            Some(index) => self.available_printings.get(index),
            None => self.available_printings.first(),
        }
    }
    
    /// Set the selected printing by index
    pub fn set_selected_printing(&mut self, print_index: usize) {
        if print_index < self.available_printings.len() {
            self.selected_printing = Some(print_index);
        }
    }
}

/// Multi-page grid preview state
#[derive(Debug, Clone)]
pub struct GridPreview {
    pub entries: Vec<PreviewEntry>,         // One per decklist entry
    pub current_page: usize,                // 0-indexed current page
    pub total_pages: usize,                 // Calculated from card count
    pub selected_entry_index: Option<usize>, // For print selection modal
}

impl GridPreview {
    /// Calculate total number of pages needed for all cards
    pub fn calculate_total_pages(&self) -> usize {
        let total_cards: usize = self.entries.iter()
            .map(|entry| entry.decklist_entry.multiple as usize)
            .sum();
        if total_cards == 0 {
            0
        } else {
            (total_cards + 8) / 9  // Ceiling division for 9 cards per page
        }
    }
    
    /// Get all grid positions for the current page
    pub fn get_current_page_positions(&self) -> Vec<(usize, &GridPosition, &PreviewEntry)> {
        let mut positions = Vec::new();
        for (entry_idx, entry) in self.entries.iter().enumerate() {
            for position in &entry.grid_positions {
                if position.page == self.current_page {
                    positions.push((entry_idx, position, entry));
                }
            }
        }
        positions.sort_by_key(|(_, pos, _)| pos.position_in_page);
        positions
    }
    
    /// Navigate to next page if possible
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.total_pages {
            self.current_page += 1;
            true
        } else {
            false
        }
    }
    
    /// Navigate to previous page if possible
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }
}

/// Page navigation state
#[derive(Debug, Clone)]
pub struct PageNavigation {
    pub current_page: usize,            // Current page being viewed
    pub total_pages: usize,             // Total pages calculated from cards
    pub can_go_prev: bool,              // Navigation state
    pub can_go_next: bool,
}

impl PageNavigation {
    pub fn new(total_pages: usize) -> Self {
        Self {
            current_page: 0,
            total_pages,
            can_go_prev: false,
            can_go_next: total_pages > 1,
        }
    }
    
    pub fn update_navigation_state(&mut self, current_page: usize) {
        self.current_page = current_page;
        self.can_go_prev = current_page > 0;
        self.can_go_next = current_page + 1 < self.total_pages;
    }
}

/// Preview mode state
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewMode {
    Hidden,           // Traditional workflow (parse → generate)
    GridPreview,      // Show 3x3 grid preview
    PrintSelection,   // Modal for selecting prints
}

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
    
    // Grid preview lifecycle
    BuildGridPreview,
    GridPreviewBuilt(Result<GridPreview, String>),
    
    // Page navigation
    NextPage,
    PrevPage,
    
    // Print selection
    ShowPrintSelection(usize),          // Entry index
    SelectPrint { 
        entry_index: usize, 
        print_index: usize 
    },
    ClosePrintSelection,
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
    
    // New preview-related fields
    grid_preview: Option<GridPreview>,
    page_navigation: Option<PageNavigation>,
    preview_mode: PreviewMode,
    is_building_preview: bool,
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
            
            // Initialize new preview fields
            grid_preview: None,
            page_navigation: None,
            preview_mode: PreviewMode::Hidden,
            is_building_preview: false,
        }
    }
}

/// Build grid preview from parsed decklist entries
async fn build_grid_preview_from_entries(entries: Vec<DecklistEntry>) -> Result<GridPreview, String> {
    let mut preview_entries = Vec::new();
    let mut card_position = 0; // Global card position counter
    
    // Process each decklist entry
    for (entry_index, decklist_entry) in entries.into_iter().enumerate() {
        log::debug!("Building preview for entry: {}x '{}'", decklist_entry.multiple, decklist_entry.name);
        
        // Search for all printings of this card
        let available_printings = match ProxyGenerator::search_card(&decklist_entry.name).await {
            Ok(search_result) => {
                log::debug!("Found {} printings for '{}'", search_result.cards.len(), decklist_entry.name);
                search_result.cards
            }
            Err(e) => {
                log::warn!("Failed to search for card '{}': {}", decklist_entry.name, e);
                return Err(format!("Failed to search for card '{}': {}", decklist_entry.name, e));
            }
        };
        
        if available_printings.is_empty() {
            return Err(format!("No printings found for card '{}'", decklist_entry.name));
        }
        
        // Determine default selected printing based on set hint from decklist
        let selected_printing = if let Some(ref decklist_set) = decklist_entry.set {
            // Try to find a printing matching the set hint
            available_printings.iter()
                .position(|card| card.set.to_lowercase() == decklist_set.to_lowercase())
        } else {
            // No set hint, default to first printing
            Some(0)
        };
        
        log::debug!("Selected printing index for '{}': {:?}", decklist_entry.name, selected_printing);
        
        // Calculate grid positions for all copies of this entry
        let mut grid_positions = Vec::new();
        for copy_number in 0..(decklist_entry.multiple as usize) {
            let page = card_position / 9; // 9 cards per page
            let position_in_page = card_position % 9;
            
            grid_positions.push(GridPosition {
                page,
                position_in_page,
                entry_index,
                copy_number,
            });
            
            card_position += 1;
        }
        
        preview_entries.push(PreviewEntry {
            decklist_entry,
            available_printings,
            selected_printing,
            grid_positions,
        });
    }
    
    // Create the grid preview
    let total_pages = if card_position == 0 {
        0
    } else {
        (card_position + 8) / 9 // Ceiling division
    };
    
    let mut grid_preview = GridPreview {
        entries: preview_entries,
        current_page: 0,
        total_pages,
        selected_entry_index: None,
    };
    
    // Update total_pages using the method for consistency
    grid_preview.total_pages = grid_preview.calculate_total_pages();
    
    log::debug!("Built grid preview with {} entries across {} pages", grid_preview.entries.len(), grid_preview.total_pages);
    
    Ok(grid_preview)
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
        
        // Grid preview lifecycle handlers
        Message::BuildGridPreview => {
            if state.parsed_cards.is_empty() {
                state.error_message = Some("No cards parsed to build preview".to_string());
                return Task::none();
            }
            
            state.is_building_preview = true;
            state.error_message = None;
            
            let cards = state.parsed_cards.clone();
            return Task::perform(
                build_grid_preview_from_entries(cards),
                Message::GridPreviewBuilt
            );
        }
        
        Message::GridPreviewBuilt(result) => {
            state.is_building_preview = false;
            
            match result {
                Ok(grid_preview) => {
                    let total_pages = grid_preview.total_pages;
                    state.page_navigation = Some(PageNavigation::new(total_pages));
                    state.grid_preview = Some(grid_preview);
                    state.preview_mode = PreviewMode::GridPreview;
                    state.display_text = format!("Grid preview built with {} pages", total_pages);
                }
                Err(error) => {
                    state.error_message = Some(error);
                    state.display_text = "Failed to build grid preview".to_string();
                }
            }
        }
        
        // Page navigation handlers
        Message::NextPage => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if grid_preview.next_page() {
                    if let Some(ref mut page_nav) = state.page_navigation {
                        page_nav.update_navigation_state(grid_preview.current_page);
                    }
                }
            }
        }
        
        Message::PrevPage => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if grid_preview.prev_page() {
                    if let Some(ref mut page_nav) = state.page_navigation {
                        page_nav.update_navigation_state(grid_preview.current_page);
                    }
                }
            }
        }
        
        // Print selection handlers
        Message::ShowPrintSelection(entry_index) => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if entry_index < grid_preview.entries.len() {
                    grid_preview.selected_entry_index = Some(entry_index);
                    state.preview_mode = PreviewMode::PrintSelection;
                }
            }
        }
        
        Message::SelectPrint { entry_index, print_index } => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if let Some(entry) = grid_preview.entries.get_mut(entry_index) {
                    entry.set_selected_printing(print_index);
                    log::debug!("Selected printing {} for entry {}", print_index, entry_index);
                    
                    // Update the corresponding DecklistEntry in parsed_cards with selected printing info
                    if let Some(selected_card) = entry.get_selected_card() {
                        // Find the matching entry in parsed_cards by name
                        if let Some(parsed_entry) = state.parsed_cards.iter_mut().find(|parsed| {
                            parsed.name.to_lowercase() == entry.decklist_entry.name.to_lowercase()
                        }) {
                            // Update the parsed entry with the selected printing's set and language
                            parsed_entry.set = Some(selected_card.set.clone());
                            parsed_entry.lang = Some(selected_card.language.clone());
                            
                            log::debug!(
                                "Updated parsed entry '{}' with selected printing: set='{}', lang='{}'", 
                                parsed_entry.name, 
                                selected_card.set, 
                                selected_card.language
                            );
                        }
                    }
                }
            }
            state.preview_mode = PreviewMode::GridPreview;
            if let Some(ref mut grid_preview) = state.grid_preview {
                grid_preview.selected_entry_index = None;
            }
        }
        
        Message::ClosePrintSelection => {
            state.preview_mode = PreviewMode::GridPreview;
            if let Some(ref mut grid_preview) = state.grid_preview {
                grid_preview.selected_entry_index = None;
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
                button(if state.is_building_preview {
                    "Building Preview..."
                } else {
                    "Build Preview"
                })
                .on_press_maybe(if state.is_building_preview {
                    None
                } else {
                    Some(Message::BuildGridPreview)
                })
                .padding(10),
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

    // Grid preview section - Multi-page preview with print selection
    // IMPLEMENTED: Basic 3x3 grid preview, page navigation, print selection modal, entry-based selection
    // TODO PHASE 3: Replace text with actual card images for true WYSIWYG preview
    // TODO PHASE 4: Advanced UI polish (hover effects, visual grouping, keyboard shortcuts)
    let grid_preview_section = if let Some(ref grid_preview) = state.grid_preview {
        match state.preview_mode {
            PreviewMode::GridPreview => {
                // Page navigation controls
                // TODO: Add direct page navigation (clickable page numbers: 1, 2, 3, ...)
                // TODO: Add keyboard shortcuts (arrow keys, Page Up/Down)
                // TODO: Add page jump input field for large decklists
                let page_nav = if let Some(ref page_navigation) = state.page_navigation {
                    row![
                        button("Previous")
                            .on_press_maybe(if page_navigation.can_go_prev {
                                Some(Message::PrevPage)
                            } else {
                                None
                            })
                            .padding(5),
                        text(format!(
                            "Page {} of {}",
                            page_navigation.current_page + 1,
                            page_navigation.total_pages
                        ))
                        .size(14),
                        button("Next")
                            .on_press_maybe(if page_navigation.can_go_next {
                                Some(Message::NextPage)
                            } else {
                                None
                            })
                            .padding(5),
                    ]
                    .spacing(10)
                } else {
                    row![]
                };

                // Get current page positions
                let current_positions = grid_preview.get_current_page_positions();
                
                // Create a 3x3 grid of cards
                // TODO: Replace text buttons with actual card images for true WYSIWYG preview
                // TODO: Add visual indicators (borders, badges) to group cards from same decklist entry
                // TODO: Add hover effects to highlight all cards from same entry
                let mut grid_rows = Vec::new();
                for row_idx in 0..3 {
                    let mut grid_row = Vec::new();
                    for col_idx in 0..3 {
                        let position_idx = row_idx * 3 + col_idx;
                        
                        if let Some((entry_idx, _position, entry)) = current_positions.get(position_idx) {
                            // Card found for this position
                            let card_name = if let Some(selected_card) = entry.get_selected_card() {
                                format!("{}\n[{}]", selected_card.name, selected_card.set.to_uppercase())
                            } else {
                                entry.decklist_entry.name.clone()
                            };
                            
                            // TODO: Replace with actual card image widget when image loading is implemented
                            // TODO: Add copy indicator (1/4, 2/4, etc.) for multi-card entries
                            let card_button = button(text(card_name)
                                .size(10))
                                .on_press(Message::ShowPrintSelection(*entry_idx))
                                .width(Length::Fixed(120.0))
                                .height(Length::Fixed(80.0))
                                .padding(5);
                            
                            grid_row.push(container(card_button).into());
                        } else {
                            // Empty slot
                            let empty_slot = container(text("Empty")
                                .size(10))
                                .width(Length::Fixed(120.0))
                                .height(Length::Fixed(80.0))
                                .padding(5);
                            
                            grid_row.push(empty_slot.into());
                        }
                    }
                    grid_rows.push(row(grid_row).spacing(5).into());
                }

                column![
                    text("Grid Preview:").size(16),
                    page_nav,
                    column(grid_rows).spacing(5),
                ]
                .spacing(10)
            }
            PreviewMode::PrintSelection => {
                // Print selection modal
                if let Some(selected_entry_idx) = grid_preview.selected_entry_index {
                    if let Some(entry) = grid_preview.entries.get(selected_entry_idx) {
                        let modal_title = format!(
                            "Select printing for {}x {}",
                            entry.decklist_entry.multiple,
                            entry.decklist_entry.name
                        );
                        
                        // Create buttons for each available printing
                        // TODO: Replace text buttons with card image thumbnails for visual selection
                        // TODO: Add visual selection indicator (highlight border, checkmark, etc.)
                        // TODO: Add set/language info overlays on thumbnails
                        // TODO: Add sorting/filtering options (by date, legality, price)
                        let print_buttons: Vec<Element<Message>> = entry
                            .available_printings
                            .iter()
                            .enumerate()
                            .map(|(print_idx, card)| {
                                let button_text = format!("{} [{}]", card.name, card.set.to_uppercase());
                                let _is_selected = entry.selected_printing == Some(print_idx);
                                
                                // TODO: Use different button style for selected printing
                                button(text(button_text).size(12))
                                    .on_press(Message::SelectPrint {
                                        entry_index: selected_entry_idx,
                                        print_index: print_idx,
                                    })
                                    .padding(8)
                                    .into()
                            })
                            .collect();

                        // TODO: Implement proper modal overlay with backdrop and centered positioning
                        // TODO: Add keyboard shortcuts (ESC to close, arrow keys to navigate)
                        // TODO: Add search/filter functionality for large numbers of printings
                        column![
                            text(modal_title).size(16),
                            button("Close").on_press(Message::ClosePrintSelection).padding(5),
                            scrollable(column(print_buttons).spacing(5)).height(Length::Fixed(300.0)),
                        ]
                        .spacing(10)
                    } else {
                        column![text("Error: Invalid entry selected")]
                    }
                } else {
                    column![text("Error: No entry selected")]
                }
            }
            PreviewMode::Hidden => column![],
        }
    } else if state.is_building_preview {
        column![text("Building preview...").size(16)]
    } else {
        column![]
    };

    let content = column![
        decklist_section,
        parsed_cards_section,
        grid_preview_section,
        pdf_status_section,
        update_section,
        error_section,
        display_section,
    ]
    .spacing(20)
    .padding(20);

    scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
