use crate::decklist::DecklistEntry;
use crate::pagination::PaginatedGrid;
use crate::scryfall::models::Card;

/// Represents a position in a grid layout
#[derive(Debug, Clone)]
pub struct GridPosition {
    pub page: usize,             // Which page this position is on
    pub position_in_page: usize, // Position within the page grid (0-8 for 3x3)
    pub entry_index: usize,      // Which decklist entry this belongs to
    pub copy_number: usize,      // Which copy of that entry (0-based)
}

/// Contains all information about a decklist entry for grid preview
#[derive(Debug, Clone)]
pub struct PreviewEntry {
    pub decklist_entry: DecklistEntry,
    pub available_printings: Vec<Card>,
    pub selected_printing: Option<usize>, // Index into available_printings
    pub grid_positions: Vec<GridPosition>,
}

impl PreviewEntry {
    pub fn new(decklist_entry: DecklistEntry, available_printings: Vec<Card>) -> Self {
        Self {
            decklist_entry,
            available_printings,
            selected_printing: None,
            grid_positions: Vec::new(),
        }
    }

    pub fn get_selected_card(&self) -> Option<&Card> {
        if let Some(selected_index) = self.selected_printing {
            self.available_printings.get(selected_index)
        } else {
            self.available_printings.first()
        }
    }

    pub fn select_printing(&mut self, index: usize) -> bool {
        if index < self.available_printings.len() {
            self.selected_printing = Some(index);
            true
        } else {
            false
        }
    }

    /// Alias for select_printing for backwards compatibility
    pub fn set_selected_printing(&mut self, index: usize) {
        self.select_printing(index);
    }
}

/// Grid preview containing all entries and navigation state
#[derive(Debug, Clone)]
pub struct GridPreview {
    pub entries: Vec<PreviewEntry>,
    pub current_page: usize,
    pub total_pages: usize,
    pub selected_entry_index: Option<usize>, // For print selection modal
    pub print_selection_grid: Option<PaginatedGrid>, // Pagination for print selection modal
}

impl GridPreview {
    pub fn new(entries: Vec<PreviewEntry>, total_pages: usize) -> Self {
        Self {
            entries,
            current_page: 0,
            total_pages,
            selected_entry_index: None,
            print_selection_grid: None,
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

    pub fn go_to_page(&mut self, page: usize) -> bool {
        if page < self.total_pages {
            self.current_page = page;
            true
        } else {
            false
        }
    }

    pub fn select_entry(&mut self, entry_index: usize) -> bool {
        if entry_index < self.entries.len() {
            self.selected_entry_index = Some(entry_index);
            true
        } else {
            false
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_entry_index = None;
    }

    pub fn get_selected_entry(&self) -> Option<&PreviewEntry> {
        if let Some(index) = self.selected_entry_index {
            self.entries.get(index)
        } else {
            None
        }
    }

    pub fn get_selected_entry_mut(&mut self) -> Option<&mut PreviewEntry> {
        if let Some(index) = self.selected_entry_index {
            self.entries.get_mut(index)
        } else {
            None
        }
    }
}

/// Page navigation state
#[derive(Debug, Clone)]
pub struct PageNavigation {
    pub current_page: usize,
    pub total_pages: usize,
}

impl PageNavigation {
    pub fn new(total_pages: usize) -> Self {
        Self {
            current_page: 0,
            total_pages,
        }
    }

    pub fn can_go_prev(&self) -> bool {
        self.current_page > 0
    }

    pub fn can_go_next(&self) -> bool {
        self.current_page < self.total_pages.saturating_sub(1)
    }

    pub fn prev_page(&mut self) -> bool {
        if self.can_go_prev() {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    pub fn next_page(&mut self) -> bool {
        if self.can_go_next() {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    pub fn go_to_page(&mut self, page: usize) -> bool {
        if page < self.total_pages {
            self.current_page = page;
            true
        } else {
            false
        }
    }

    pub fn update_navigation_state(&mut self, current_page: usize) {
        self.current_page = current_page;
    }
}

/// Individual image position in the grid layout
#[derive(Debug, Clone)]
pub struct GridImage {
    pub entry_index: usize,      // Which decklist entry this came from
    pub copy_number: usize,      // Which copy of that entry (0-based)
    pub page: usize,             // Which page this appears on
    pub position_in_page: usize, // Position within the page grid (0-8)
}
