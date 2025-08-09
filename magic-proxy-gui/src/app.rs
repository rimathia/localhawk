use iced::widget::{
    button, column, container, image, pick_list, row, scrollable, text, text_editor,
};
use iced::{Element, Length, Task};
use magic_proxy_core::{
    BackgroundLoadHandle, BackgroundLoadProgress, Card, DecklistEntry, DoubleFaceMode,
    LoadingPhase, PdfOptions, ProxyGenerator, force_update_card_lookup, get_cached_image_bytes,
    get_card_name_cache_info, get_image_cache_info, start_background_image_loading,
};
use rfd::AsyncFileDialog;

// Constants for grid preview card dimensions (maintaining Magic card aspect ratio: 480:680 = ~0.706)
const GRID_CARD_WIDTH: f32 = 200.0;
const GRID_CARD_HEIGHT: f32 = 283.3; // 200.0 / 0.7058823529 ≈ 283.3

// Constants for print selection modal thumbnail dimensions (same size as main grid for consistency)
const THUMBNAIL_WIDTH: f32 = GRID_CARD_WIDTH;
const THUMBNAIL_HEIGHT: f32 = GRID_CARD_HEIGHT;

// Constants for print selection modal grid dimensions
const PRINT_SELECTION_COLUMNS: usize = 5;
const PRINT_SELECTION_ROWS: usize = 3;
const PRINTS_PER_PAGE: usize = PRINT_SELECTION_COLUMNS * PRINT_SELECTION_ROWS;

/// Reusable paginated card grid component
#[derive(Debug, Clone)]
pub struct PaginatedCardGrid {
    pub current_page: usize,
    pub total_items: usize,
    pub items_per_page: usize,
    pub _grid_columns: usize, // Keep for potential future use
    pub _grid_rows: usize,    // Keep for potential future use
}

impl PaginatedCardGrid {
    pub fn new(
        total_items: usize,
        items_per_page: usize,
        grid_columns: usize,
        grid_rows: usize,
    ) -> Self {
        let _total_pages = if total_items == 0 {
            1
        } else {
            (total_items + items_per_page - 1) / items_per_page // Ceiling division
        };

        Self {
            current_page: 0,
            total_items,
            items_per_page,
            _grid_columns: grid_columns,
            _grid_rows: grid_rows,
        }
    }

    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 {
            1
        } else {
            (self.total_items + self.items_per_page - 1) / self.items_per_page
        }
    }

    pub fn can_go_prev(&self) -> bool {
        self.current_page > 0
    }

    pub fn can_go_next(&self) -> bool {
        self.current_page < self.total_pages() - 1
    }

    pub fn prev_page(&mut self) {
        if self.can_go_prev() {
            self.current_page -= 1;
        }
    }

    pub fn next_page(&mut self) {
        if self.can_go_next() {
            self.current_page += 1;
        }
    }

    pub fn get_current_page_items(&self) -> (usize, usize) {
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.total_items);
        (start, end)
    }

    /// Create navigation controls for the paginated grid
    pub fn create_navigation_controls<'a>(
        &self,
        prev_message: Message,
        next_message: Message,
    ) -> Element<'a, Message> {
        if self.total_pages() <= 1 {
            // No navigation needed for single page
            return row![].into();
        }

        row![
            button("Previous")
                .on_press_maybe(if self.can_go_prev() {
                    Some(prev_message)
                } else {
                    None
                })
                .padding(5),
            text(format!(
                "Page {} of {}",
                self.current_page + 1,
                self.total_pages()
            ))
            .size(14),
            button("Next")
                .on_press_maybe(if self.can_go_next() {
                    Some(next_message)
                } else {
                    None
                })
                .padding(5),
        ]
        .spacing(10)
        .into()
    }
}

/// Individual card position in the grid layout
#[derive(Debug, Clone, PartialEq)]
pub struct GridPosition {
    pub page: usize,             // Which page this position is on
    pub position_in_page: usize, // 0-8 position within 3x3 grid
    pub entry_index: usize,      // Back-reference to parent entry
    pub copy_number: usize,      // 1st, 2nd, 3rd, 4th copy of entry
}

/// Represents one decklist entry with all its printings and positions
#[derive(Debug, Clone)]
pub struct PreviewEntry {
    pub decklist_entry: DecklistEntry, // Original "4x Lightning Bolt [LEA]"
    pub available_printings: Vec<Card>, // All printings found from search
    pub selected_printing: Option<usize>, // Index into available_printings
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
    pub entries: Vec<PreviewEntry>,          // One per decklist entry
    pub current_page: usize,                 // 0-indexed current page
    pub total_pages: usize,                  // Calculated from card count
    pub selected_entry_index: Option<usize>, // For print selection modal
    pub print_selection_grid: Option<PaginatedCardGrid>, // Pagination for print selection modal
}

impl GridPreview {
    /// Calculate total number of pages needed for all cards
    pub fn _calculate_total_pages(&self) -> usize {
        let total_cards: usize = self
            .entries
            .iter()
            .map(|entry| {
                if let Some(selected_card) = entry.get_selected_card() {
                    _calculate_actual_card_count(&entry.decklist_entry, selected_card)
                } else {
                    entry.decklist_entry.multiple as usize // Fallback if no card found
                }
            })
            .sum();
        if total_cards == 0 {
            0
        } else {
            (total_cards + 8) / 9 // Ceiling division for 9 cards per page
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
    pub current_page: usize, // Current page being viewed
    pub total_pages: usize,  // Total pages calculated from cards
    pub can_go_prev: bool,   // Navigation state
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
    Hidden,         // Traditional workflow (parse → generate)
    GridPreview,    // Show 3x3 grid preview
    PrintSelection, // Modal for selecting prints
}

#[derive(Debug, Clone)]
pub enum Message {
    DecklistAction(text_editor::Action),
    ParseDecklist,
    DecklistParsed(Vec<DecklistEntry>),
    ClearDecklist,
    GenerateAll, // New: Parse + Generate + Save in one step
    GeneratePdf,
    PdfGenerated(Result<Vec<u8>, String>),
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
    ShowPrintSelection(usize), // Entry index
    SelectPrint {
        entry_index: usize,
        print_index: usize,
    },
    ClosePrintSelection,

    // Print selection pagination
    PrintSelectionPrevPage,
    PrintSelectionNextPage,

    // Background image loading (now using core library)
    PollBackgroundProgress,
}

pub struct AppState {
    display_text: String,
    decklist_content: text_editor::Content,
    parsed_cards: Vec<DecklistEntry>,
    parsed_cards_aligned_text: text_editor::Content, // Line-by-line aligned output
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

    // Background image loading (now using core library)
    background_load_handle: Option<BackgroundLoadHandle>,
    latest_background_progress: Option<BackgroundLoadProgress>,

    // Auto-continue to PDF generation after parsing
    auto_generate_after_parse: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            display_text: "Welcome to Magic Card Proxy Generator!\nParsing includes fuzzy matching, set/language awareness, and card name resolution.".to_string(),
            decklist_content: text_editor::Content::with_text(
                "\n1 Counterspell [7ED]\n// comments are ignored\n1 Memory Lapse [ja]\n1 kabira takedown\n1 kabira plateau\n1 cut // ribbons (pakh)\n1 Gisela, the Broken Blade\n1 Bruna, the Fading Light",
            ),
            parsed_cards: Vec::new(),
            parsed_cards_aligned_text: text_editor::Content::new(),
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

            // Initialize background loading fields
            background_load_handle: None,
            latest_background_progress: None,

            // Initialize auto-continue flag
            auto_generate_after_parse: false,
        }
    }
}

/// Calculate the actual number of images that will be generated for a decklist entry
/// considering its face mode and whether the card has a back face
fn _calculate_actual_card_count(entry: &DecklistEntry, card: &Card) -> usize {
    let base_count = entry.multiple as usize;

    match entry.face_mode {
        DoubleFaceMode::FrontOnly => {
            // Always 1 image per copy (front only)
            base_count
        }
        DoubleFaceMode::BackOnly => {
            // 1 image per copy (back if it exists, otherwise front)
            base_count
        }
        DoubleFaceMode::BothSides => {
            // 1 or 2 images per copy depending on whether card has back
            if card.border_crop_back.is_some() {
                base_count * 2 // Front + back
            } else {
                base_count // Front only
            }
        }
    }
}

/// Select the best card from available printings using the same logic as PDF generation
fn select_card_from_printings(
    available_printings: &[Card],
    entry: &DecklistEntry,
) -> Option<usize> {
    // Use the same selection logic as PDF generation for consistency
    available_printings.iter().position(|c| {
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
}

/// Build aligned text output: start with original decklist, replace successfully parsed lines
/// Uses current parsed_cards state (which may have updated printings)
fn build_aligned_parsed_output(input_text: &str, parsed_cards: &[DecklistEntry]) -> String {
    let input_lines: Vec<&str> = input_text.lines().collect();
    let mut output_lines: Vec<String> = input_lines.iter().map(|line| line.to_string()).collect();

    // Replace lines where we successfully parsed something
    for entry in parsed_cards {
        if let Some(line_num) = entry.source_line_number {
            if line_num < output_lines.len() {
                let set_info = if let Some(set) = &entry.set {
                    format!(" • Set: {}", set.to_uppercase())
                } else {
                    String::new()
                };
                let lang_info = if let Some(lang) = &entry.lang {
                    format!(" • Lang: {}", lang.to_uppercase())
                } else {
                    String::new()
                };
                let face_info = match entry.face_mode {
                    DoubleFaceMode::FrontOnly => " • Face: Front only".to_string(),
                    DoubleFaceMode::BackOnly => " • Face: Back only".to_string(),
                    DoubleFaceMode::BothSides => " • Face: Both sides".to_string(),
                };

                output_lines[line_num] = format!(
                    "✓ {}x {}{}{}{}",
                    entry.multiple, entry.name, set_info, lang_info, face_info
                );
            }
        }
    }

    output_lines.join("\n")
}

/// Individual image that will appear in the grid (exactly matching PDF generation)
#[derive(Debug, Clone)]
pub struct GridImage {
    pub entry_index: usize,      // Which decklist entry this came from
    pub copy_number: usize,      // Which copy of that entry (0-based)
    pub _image_index: usize, // Which image within that copy (for double-faced cards) - keep for future use
    pub _card: Card,         // The actual card - keep for future use
    pub image_url: String,   // The URL of the image to display
    pub page: usize,         // Which page this appears on
    pub position_in_page: usize, // Position within the 3x3 grid (0-8)
}

/// Build grid preview using the exact same logic as PDF generation
async fn build_grid_preview_from_entries(
    entries: Vec<DecklistEntry>,
) -> Result<GridPreview, String> {
    let mut all_images = Vec::new();
    let mut card_position = 0;

    // Store search results to avoid duplicate API calls
    let mut search_results = Vec::new();

    // First pass: Process each decklist entry and store search results
    for (entry_index, decklist_entry) in entries.iter().enumerate() {
        log::debug!(
            "Processing entry for grid: {}x '{}' with face mode {:?}",
            decklist_entry.multiple,
            decklist_entry.name,
            decklist_entry.face_mode
        );

        // Search for the card to get printings (same as PDF generation)
        let search_result = match ProxyGenerator::search_card(&decklist_entry.name).await {
            Ok(result) => result,
            Err(e) => {
                log::warn!("Failed to search for card '{}': {}", decklist_entry.name, e);
                return Err(format!(
                    "Failed to search for card '{}': {}",
                    decklist_entry.name, e
                ));
            }
        };

        if search_result.cards.is_empty() {
            return Err(format!(
                "No printings found for card '{}'",
                decklist_entry.name
            ));
        }

        // Store search result for later reuse
        search_results.push(search_result.clone());

        // Use the same card selection logic as PDF generation
        let selected_card = select_card_from_printings(&search_result.cards, decklist_entry)
            .and_then(|idx| search_result.cards.get(idx))
            .or_else(|| search_result.cards.first())
            .cloned();

        let card = match selected_card {
            Some(card) => card,
            None => {
                return Err(format!(
                    "No suitable card found for entry '{}'",
                    decklist_entry.name
                ));
            }
        };

        log::debug!(
            "Selected card: '{}' ({}) [{}] for entry '{}' ({} total printings)",
            card.name,
            card.set.to_uppercase(),
            card.language,
            decklist_entry.name,
            search_result.cards.len()
        );

        // Generate the actual images based on face mode - using the SAME logic as PDF generation
        for copy_number in 0..decklist_entry.multiple {
            let image_urls =
                ProxyGenerator::get_image_urls_for_face_mode(&card, &decklist_entry.face_mode);

            for (image_index, image_url) in image_urls.into_iter().enumerate() {
                let page = card_position / 9; // 9 cards per page
                let position_in_page = card_position % 9;

                all_images.push(GridImage {
                    entry_index,
                    copy_number: copy_number as usize,
                    _image_index: image_index,
                    _card: card.clone(),
                    image_url,
                    page,
                    position_in_page,
                });

                card_position += 1;

                log::debug!(
                    "Added grid image: entry={}, copy={}, image={}, url={}, page={}, pos={}",
                    entry_index,
                    copy_number,
                    image_index,
                    all_images.last().unwrap().image_url,
                    page,
                    position_in_page
                );
            }
        }
    }

    let total_pages = if all_images.is_empty() {
        0
    } else {
        (all_images.len() + 8) / 9 // Ceiling division
    };

    // Second pass: Convert to the old format using cached search results (no duplicate API calls)
    let mut preview_entries = Vec::new();
    for (entry_index, decklist_entry) in entries.into_iter().enumerate() {
        // Reuse search results from first pass instead of making another API call
        let available_printings = if let Some(search_result) = search_results.get(entry_index) {
            search_result.cards.clone()
        } else {
            Vec::new() // Fallback if something went wrong
        };

        let selected_printing = select_card_from_printings(&available_printings, &decklist_entry);

        // Create grid positions for this entry by finding all images that belong to it
        let grid_positions: Vec<GridPosition> = all_images
            .iter()
            .filter(|img| img.entry_index == entry_index)
            .map(|img| GridPosition {
                page: img.page,
                position_in_page: img.position_in_page,
                entry_index: img.entry_index,
                copy_number: img.copy_number,
            })
            .collect();

        preview_entries.push(PreviewEntry {
            decklist_entry,
            available_printings,
            selected_printing,
            grid_positions,
        });
    }

    let grid_preview = GridPreview {
        entries: preview_entries,
        current_page: 0,
        total_pages,
        selected_entry_index: None,
        print_selection_grid: None,
    };

    log::debug!(
        "Built grid preview with {} total images across {} pages (exactly matching PDF)",
        all_images.len(),
        total_pages
    );

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

            // Parse and resolve decklist with global caches and current face mode setting
            let current_face_mode = state.double_face_mode.clone();
            return Task::perform(
                async move {
                    match ProxyGenerator::parse_and_resolve_decklist(
                        &decklist_text,
                        current_face_mode,
                    )
                    .await
                    {
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
                    "  GUI card: {}x '{}' [set: {:?}, lang: {:?}, face_mode: {:?}]",
                    card.multiple,
                    card.name,
                    card.set,
                    card.lang,
                    card.face_mode
                );
            }
            state.parsed_cards = cards.clone();
            state.error_message = None;
            state.display_text = format!(
                "Parsed {} cards successfully! Loading images and building preview...",
                state.parsed_cards.len()
            );

            // Build aligned text output for the right panel
            let aligned_text =
                build_aligned_parsed_output(&state.decklist_content.text(), &state.parsed_cards);
            state.parsed_cards_aligned_text = text_editor::Content::with_text(&aligned_text);

            // Start background image loading immediately after parsing (now using core library)
            if !cards.is_empty() {
                // Start background loading in core library
                let handle = start_background_image_loading(cards.clone());
                state.background_load_handle = Some(handle);

                let mut tasks = vec![
                    Task::perform(async { () }, |_| Message::PollBackgroundProgress),
                    Task::perform(async { () }, |_| Message::BuildGridPreview),
                ];

                // If GenerateAll was triggered, auto-continue to PDF generation
                if state.auto_generate_after_parse {
                    state.auto_generate_after_parse = false; // Reset flag
                    tasks.push(Task::perform(async { () }, |_| Message::GeneratePdf));
                }

                return Task::batch(tasks);
            }
        }
        Message::PollBackgroundProgress => {
            if let Some(handle) = state.background_load_handle.as_mut() {
                if let Some(progress) = handle.try_get_progress() {
                    log::debug!("Background progress update: {:?}", progress);
                    state.latest_background_progress = Some(progress.clone());

                    // Update display text with progress
                    let progress_text = match progress.phase {
                        LoadingPhase::Selected => {
                            format!(
                                "Loading selected images: {}/{} entries, {} images cached...",
                                progress.current_entry,
                                progress.total_entries,
                                progress.selected_loaded
                            )
                        }
                        LoadingPhase::Alternatives => {
                            format!(
                                "Loading alternative images: {}/{} alternatives cached...",
                                progress.alternatives_loaded, progress.total_alternatives
                            )
                        }
                        LoadingPhase::Completed => {
                            format!(
                                "All images loaded! {} selected + {} alternatives = {} total images.",
                                progress.selected_loaded,
                                progress.alternatives_loaded,
                                progress.selected_loaded + progress.alternatives_loaded
                            )
                        }
                    };
                    state.display_text = progress_text;

                    // Show any errors
                    if !progress.errors.is_empty() {
                        let error_msg = format!(
                            "Loading completed with {} error(s): {}",
                            progress.errors.len(),
                            progress.errors.join("; ")
                        );
                        state.error_message = Some(error_msg);
                    }
                }

                // Check if loading is finished
                if handle.is_finished() {
                    log::debug!("Background loading task finished");
                    state.background_load_handle = None;
                } else {
                    // Continue polling
                    return Task::perform(
                        async {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        },
                        |_| Message::PollBackgroundProgress,
                    );
                }
            }
        }
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
                Message::GridPreviewBuilt,
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
        Message::ShowPrintSelection(entry_index) => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if entry_index < grid_preview.entries.len() {
                    grid_preview.selected_entry_index = Some(entry_index);

                    // Initialize pagination grid for print selection
                    let entry = &grid_preview.entries[entry_index];
                    let total_printings = entry.available_printings.len();
                    grid_preview.print_selection_grid = Some(PaginatedCardGrid::new(
                        total_printings,
                        PRINTS_PER_PAGE,
                        PRINT_SELECTION_COLUMNS,
                        PRINT_SELECTION_ROWS,
                    ));

                    state.preview_mode = PreviewMode::PrintSelection;
                }
            }
        }
        Message::SelectPrint {
            entry_index,
            print_index,
        } => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if let Some(entry) = grid_preview.entries.get_mut(entry_index) {
                    entry.set_selected_printing(print_index);
                    log::debug!(
                        "Selected printing {} for entry {}",
                        print_index,
                        entry_index
                    );

                    // Update the corresponding DecklistEntry in parsed_cards with selected printing info
                    if let Some(selected_card) = entry.get_selected_card() {
                        // Find the matching entry in parsed_cards by name
                        log::debug!(
                            "Looking for match: grid entry name='{}', checking against {} parsed entries",
                            entry.decklist_entry.name,
                            state.parsed_cards.len()
                        );

                        for parsed in &state.parsed_cards {
                            log::debug!("  Parsed entry: '{}'", parsed.name);
                        }

                        if let Some(parsed_entry) = state.parsed_cards.iter_mut().find(|parsed| {
                            parsed.name.to_lowercase() == entry.decklist_entry.name.to_lowercase()
                                && parsed.face_mode == entry.decklist_entry.face_mode
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
                        } else {
                            log::warn!(
                                "Could not find matching parsed entry for grid entry '{}'",
                                entry.decklist_entry.name
                            );
                        }
                    }
                }
            }

            // Rebuild aligned text output after print selection
            let aligned_text =
                build_aligned_parsed_output(&state.decklist_content.text(), &state.parsed_cards);
            state.parsed_cards_aligned_text = text_editor::Content::with_text(&aligned_text);

            state.preview_mode = PreviewMode::GridPreview;
            if let Some(ref mut grid_preview) = state.grid_preview {
                grid_preview.selected_entry_index = None;
            }
        }
        Message::ClosePrintSelection => {
            state.preview_mode = PreviewMode::GridPreview;
            if let Some(ref mut grid_preview) = state.grid_preview {
                grid_preview.selected_entry_index = None;
                grid_preview.print_selection_grid = None;
            }
        }
        Message::PrintSelectionPrevPage => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if let Some(ref mut print_grid) = grid_preview.print_selection_grid {
                    print_grid.prev_page();
                }
            }
        }
        Message::PrintSelectionNextPage => {
            if let Some(ref mut grid_preview) = state.grid_preview {
                if let Some(ref mut print_grid) = grid_preview.print_selection_grid {
                    print_grid.next_page();
                }
            }
        }
        Message::ClearDecklist => {
            state.decklist_content = text_editor::Content::new();
            state.parsed_cards.clear();
            state.parsed_cards_aligned_text = text_editor::Content::new();
            state.error_message = None;
            state.display_text = "Decklist cleared!".to_string();

            // Clear preview and navigation state
            state.grid_preview = None;
            state.page_navigation = None;
            state.preview_mode = PreviewMode::Hidden;
            state.is_building_preview = false;

            // Clear background loading state
            if let Some(handle) = state.background_load_handle.take() {
                handle.cancel();
            }
            state.latest_background_progress = None;
        }
        Message::GenerateAll => {
            // Set flag to auto-continue to PDF generation after parsing
            state.auto_generate_after_parse = true;

            // Trigger parsing (PDF generation will be auto-triggered in DecklistParsed handler)
            return update(state, Message::ParseDecklist);
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
                                // Use unified card selection logic (same as preview)
                                if let Some(selected_index) =
                                    select_card_from_printings(&search_result.cards, &entry)
                                {
                                    let card = search_result
                                        .cards
                                        .into_iter()
                                        .nth(selected_index)
                                        .unwrap();
                                    log::debug!(
                                        "Selected card: '{}' ({}) [{}]",
                                        card.name,
                                        card.set.to_uppercase(),
                                        card.language
                                    );
                                    // Use the fully resolved face mode from parse_and_resolve_decklist
                                    log::debug!(
                                        "Entry details: '{}' [set: {:?}, lang: {:?}, face_mode: {:?}]",
                                        entry.name,
                                        entry.set,
                                        entry.lang,
                                        entry.face_mode
                                    );

                                    // No need for complex logic - the core library already resolved everything
                                    let face_mode = entry.face_mode.clone();
                                    log::debug!(
                                        "Using resolved face mode for '{}': {:?}",
                                        entry.name,
                                        face_mode
                                    );

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
                    state.display_text = format!(
                        "PDF generated successfully! {} bytes - Opening save dialog...",
                        pdf_data.len()
                    );

                    // Auto-trigger save dialog after successful PDF generation
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
                Err(error) => {
                    state.error_message = Some(error);
                    state.display_text = "PDF generation failed!".to_string();
                }
            }
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
    // Left side: Decklist input (text field only)
    let decklist_input_section = column![
        text("Decklist Parser:").size(18),
        text("Paste your decklist below (supports various formats):").size(14),
        text_editor(&state.decklist_content)
            .on_action(Message::DecklistAction)
            .height(Length::Fixed(400.0))
            .width(600.0) // Increased width to accommodate longer parsed entries
            .font(iced::Font::MONOSPACE), // Use monospace font for better alignment with parsed output
    ]
    .spacing(10)
    .width(Length::Fixed(650.0)); // Container width slightly larger than text field

    // Button row: independent width for proper spacing
    let button_row = row![
        button(if state.is_generating_pdf && state.is_parsing {
            "Generating PDF..."
        } else {
            "Generate & Save PDF"
        })
        .on_press_maybe(if state.is_generating_pdf || state.is_parsing {
            None
        } else {
            Some(Message::GenerateAll)
        })
        .padding(10),
        button(if state.is_parsing {
            "Parsing & Building Preview..."
        } else {
            "Parse & Preview Decklist"
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
        text("Face Mode:").size(14),
        pick_list(
            DoubleFaceMode::all(),
            Some(state.double_face_mode.clone()),
            Message::DoubleFaceModeChanged,
        )
        .width(Length::Fixed(GRID_CARD_WIDTH)),
    ]
    .spacing(10);

    // Right side: Parsed cards display (aligned with input) - using text widget for display-only content
    let parsed_cards_section = if !state.parsed_cards.is_empty() {
        let parsed_text = state.parsed_cards_aligned_text.text();
        column![
            text(format!("Parsed Cards ({}):", state.parsed_cards.len())).size(18),
            text("Resolved names, sets, languages, and face modes:").size(14),
            // Container styled to match text_editor appearance but using text widget to avoid greyed-out look
            container(
                scrollable(
                    text(parsed_text)
                        .font(iced::Font::MONOSPACE) // Use monospace font for better alignment
                        .size(16)
                        .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(20.0))) // Match text_editor line height
                )
                .height(Length::Fill)
            )
            .style(|_theme| container::Style {
                background: Some(iced::Color::WHITE.into()),
                border: iced::Border {
                    color: iced::Color::from_rgb(0.5, 0.5, 0.5),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .padding(8)
            .height(Length::Fixed(400.0))
            .width(600.0), // Same width as input text field
        ]
        .spacing(10)
        .width(Length::Fixed(650.0)) // Same container width as input section
    } else {
        column![
            text("Parsed Cards:").size(18),
            text("Resolved cards will appear here after parsing:").size(14),
            // Empty placeholder with same styling
            container(
                text("Resolved cards will appear here after parsing...")
                    .font(iced::Font::MONOSPACE)
                    .size(14)
                    .color(iced::Color::from_rgb(0.6, 0.6, 0.6))
            )
            .style(|_theme| container::Style {
                background: Some(iced::Color::from_rgb(0.98, 0.98, 0.98).into()),
                border: iced::Border {
                    color: iced::Color::from_rgb(0.5, 0.5, 0.5),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .padding(8)
            .height(Length::Fixed(400.0))
            .width(600.0),
        ]
        .spacing(10)
        .width(Length::Fixed(650.0)) // Same container width as input section
    };

    // Input section: side-by-side decklist input and parsed cards with minimal gap for visual alignment
    let input_section = row![decklist_input_section, parsed_cards_section,].spacing(5);

    // Combined top section: input + button row below
    let top_section = column![input_section, button_row,].spacing(15);

    let pdf_status_section = if state.is_generating_pdf {
        column![text("Generating PDF and opening save dialog...").size(16),].spacing(5)
    } else if let Some(pdf_data) = &state.generated_pdf {
        column![
            text("PDF Generated!").size(16),
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
                        "Card names: {} cached, last updated: {}",
                        count,
                        timestamp
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap_or_else(|_| "Unknown".to_string())
                    )
                })
                .unwrap_or_else(|| "No card name cache found".to_string())
        )
        .size(12),
        text({
            let (count, size_mb) = get_image_cache_info();
            format!("Images: {} cached, {:.1} MB", count, size_mb)
        })
        .size(12),
    ]
    .spacing(5);

    // Always-visible 3x3 grid section - shows empty placeholders before parsing, gets populated as cards are processed
    let grid_preview_section = {
        match state.preview_mode {
            PreviewMode::GridPreview | PreviewMode::Hidden => {
                // Always show 3x3 grid layout - Default mode (both GridPreview and Hidden show grid)

                // Page navigation controls (only show if we have parsed cards and multiple pages)
                let page_nav = if let Some(ref _grid_preview) = state.grid_preview {
                    if let Some(ref page_navigation) = state.page_navigation {
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
                    }
                } else {
                    row![]
                };

                // Get current page positions or use empty state
                let current_positions = if let Some(ref grid_preview) = state.grid_preview {
                    grid_preview.get_current_page_positions()
                } else {
                    Vec::new() // Empty state - will show all empty placeholders
                };

                // Create a 3x3 grid of cards
                let mut grid_rows = Vec::new();
                for row_idx in 0..3 {
                    let mut grid_row = Vec::new();
                    for col_idx in 0..3 {
                        let position_idx = row_idx * 3 + col_idx;

                        if let Some((entry_idx, _position, entry)) =
                            current_positions.get(position_idx)
                        {
                            // Try to get cached image, fallback to text if not available
                            let card_widget = if let Some(selected_card) = entry.get_selected_card()
                            {
                                // Get all image URLs that would be generated for this entry's face mode
                                let image_urls = ProxyGenerator::get_image_urls_for_face_mode(
                                    selected_card,
                                    &entry.decklist_entry.face_mode,
                                );

                                // Find which copy and which image within that copy this position represents
                                let images_per_copy = image_urls.len();
                                let total_images_for_entry =
                                    entry.decklist_entry.multiple as usize * images_per_copy;

                                // Find the position of this grid slot relative to this entry's images
                                let entry_positions: Vec<&GridPosition> = entry
                                    .grid_positions
                                    .iter()
                                    .filter(|pos| {
                                        if let Some(ref grid_preview) = state.grid_preview {
                                            pos.page == grid_preview.current_page
                                        } else {
                                            pos.page == 0 // Default to page 0 if no grid preview yet
                                        }
                                    })
                                    .collect();

                                if let Some(relative_pos) = entry_positions
                                    .iter()
                                    .position(|pos| pos.position_in_page == position_idx)
                                {
                                    let image_url = if relative_pos < total_images_for_entry {
                                        let image_index = relative_pos % images_per_copy;
                                        image_urls
                                            .get(image_index)
                                            .unwrap_or(&selected_card.border_crop)
                                    } else {
                                        &selected_card.border_crop // Fallback
                                    };

                                    if let Some(image_bytes) = get_cached_image_bytes(image_url) {
                                        // Display the correct image based on face mode and position
                                        let image_handle = image::Handle::from_bytes(image_bytes);
                                        button(
                                            image::Image::<image::Handle>::new(image_handle)
                                                .width(Length::Fixed(GRID_CARD_WIDTH))
                                                .height(Length::Fixed(GRID_CARD_HEIGHT)),
                                        )
                                        .on_press(Message::ShowPrintSelection(*entry_idx))
                                        .width(Length::Fixed(GRID_CARD_WIDTH))
                                        .height(Length::Fixed(GRID_CARD_HEIGHT))
                                        .padding(0) // No padding for seamless grid
                                    } else {
                                        // Fallback to text while image loads
                                        let face_info = if image_url == &selected_card.border_crop {
                                            "Front"
                                        } else {
                                            "Back"
                                        };
                                        button(
                                            text(format!(
                                                "{}\n[{}]\n{}\nLoading...",
                                                selected_card.name,
                                                selected_card.set.to_uppercase(),
                                                face_info
                                            ))
                                            .size(8),
                                        )
                                        .on_press(Message::ShowPrintSelection(*entry_idx))
                                        .width(Length::Fixed(GRID_CARD_WIDTH))
                                        .height(Length::Fixed(GRID_CARD_HEIGHT))
                                        .padding(0)
                                    }
                                } else {
                                    // Position not found, show fallback
                                    button(
                                        text(format!("Error\n{}", entry.decklist_entry.name))
                                            .size(9),
                                    )
                                    .on_press(Message::ShowPrintSelection(*entry_idx))
                                    .width(Length::Fixed(GRID_CARD_WIDTH))
                                    .height(Length::Fixed(GRID_CARD_HEIGHT))
                                    .padding(0)
                                }
                            } else {
                                // No card selected, show entry name
                                button(text(entry.decklist_entry.name.clone()).size(10))
                                    .on_press(Message::ShowPrintSelection(*entry_idx))
                                    .width(Length::Fixed(GRID_CARD_WIDTH))
                                    .height(Length::Fixed(GRID_CARD_HEIGHT))
                                    .padding(0)
                            };

                            grid_row.push(container(card_widget).into());
                        } else {
                            // Empty slot - show visual placeholder only (no text)
                            let empty_slot = container(text(""))
                                .width(Length::Fixed(GRID_CARD_WIDTH))
                                .height(Length::Fixed(GRID_CARD_HEIGHT))
                                .center_x(Length::Fixed(GRID_CARD_WIDTH))
                                .center_y(Length::Fixed(GRID_CARD_HEIGHT));

                            grid_row.push(empty_slot.into());
                        }
                    }
                    grid_rows.push(row(grid_row).spacing(0).into()); // No spacing between cards
                }

                let grid_title = if state.is_building_preview {
                    "Grid Preview: Building..."
                } else if state.grid_preview.is_some() {
                    "Grid Preview:"
                } else {
                    "PDF Preview (3x3 grid):"
                };

                column![
                    text(grid_title).size(16),
                    page_nav,
                    column(grid_rows).spacing(0),
                    // Generate PDF button (only show when we have parsed cards)
                    if state.parsed_cards.is_empty() {
                        Element::from(column![])
                    } else {
                        Element::from(
                            row![
                                button(if state.is_generating_pdf {
                                    "Generating PDF..."
                                } else {
                                    "Generate & Save PDF from Preview"
                                })
                                .on_press_maybe(if state.is_generating_pdf {
                                    None
                                } else {
                                    Some(Message::GeneratePdf)
                                })
                                .padding(10),
                            ]
                            .spacing(10),
                        )
                    },
                ]
                .spacing(10)
            }
            PreviewMode::PrintSelection => {
                // Print selection modal - only show when explicitly in this mode
                if let Some(ref grid_preview) = state.grid_preview {
                    if let Some(selected_entry_idx) = grid_preview.selected_entry_index {
                        if let Some(entry) = grid_preview.entries.get(selected_entry_idx) {
                            let modal_title = format!(
                                "Select printing for {}x {}",
                                entry.decklist_entry.multiple, entry.decklist_entry.name
                            );

                            // Get pagination info from the grid
                            let print_grid = grid_preview.print_selection_grid.as_ref().unwrap();
                            let (start_idx, end_idx) = print_grid.get_current_page_items();

                            // Create pagination controls
                            let page_nav = print_grid.create_navigation_controls(
                                Message::PrintSelectionPrevPage,
                                Message::PrintSelectionNextPage,
                            );

                            // Create buttons only for current page printings (much faster!)
                            let print_buttons: Vec<Element<Message>> = entry
                                .available_printings
                                .iter()
                                .skip(start_idx)
                                .take(end_idx - start_idx)
                                .enumerate()
                                .map(|(page_relative_idx, card)| {
                                    let actual_print_idx = start_idx + page_relative_idx; // Convert back to global index
                                    let is_selected =
                                        entry.selected_printing == Some(actual_print_idx);

                                    // Show only the image - cleaner and more space-efficient
                                    let button_content: Element<Message> =
                                        if let Some(image_bytes) =
                                            get_cached_image_bytes(&card.border_crop)
                                        {
                                            // Show actual card image thumbnail only
                                            let image_handle =
                                                image::Handle::from_bytes(image_bytes);
                                            image::Image::<image::Handle>::new(image_handle)
                                                .width(Length::Fixed(THUMBNAIL_WIDTH))
                                                .height(Length::Fixed(THUMBNAIL_HEIGHT))
                                                .into()
                                        } else {
                                            // Minimal fallback while image loads
                                            container(text("...").size(12))
                                                .width(Length::Fixed(THUMBNAIL_WIDTH))
                                                .height(Length::Fixed(THUMBNAIL_HEIGHT))
                                                .center_x(Length::Fill)
                                                .center_y(Length::Fill)
                                                .into()
                                        };

                                    // Use different style for selected printing with tooltip
                                    let btn = button(button_content)
                                        .on_press(Message::SelectPrint {
                                            entry_index: selected_entry_idx,
                                            print_index: actual_print_idx,
                                        })
                                        .padding(if is_selected { 3 } else { 0 }); // Minimal padding, selected gets slight border

                                    btn.into()
                                })
                                .collect();

                            // Create grid layout for current page printings using configurable dimensions
                            let mut print_rows = Vec::new();
                            let mut current_row = Vec::new();

                            for (i, button) in print_buttons.into_iter().enumerate() {
                                current_row.push(button);

                                if current_row.len() == PRINT_SELECTION_COLUMNS
                                    || i == (end_idx - start_idx) - 1
                                {
                                    // Complete row or last item of current page - no spacing for tighter grid
                                    print_rows.push(row(current_row).spacing(0).into());
                                    current_row = Vec::new();
                                }
                            }

                            column![
                            text(modal_title).size(16),
                            button("Close")
                                .on_press(Message::ClosePrintSelection)
                                .padding(5),
                            page_nav,
                            text(format!("Click on a card image to select that printing ({} total printings):", entry.available_printings.len())).size(12),
                            column(print_rows).spacing(0),
                        ]
                        .spacing(10)
                        } else {
                            column![text("Error: Invalid entry selected")]
                        }
                    } else {
                        column![text("Error: No entry selected")]
                    }
                } else {
                    column![text("Error: No grid preview available")]
                }
            }
        }
    };

    let content = column![
        top_section,
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
