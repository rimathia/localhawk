/// Generic pagination utility for managing paged content
#[derive(Debug, Clone)]
pub struct PaginatedView<T> {
    pub items: Vec<T>,
    pub current_page: usize,
    pub items_per_page: usize,
}

impl<T> PaginatedView<T> {
    pub fn new(items: Vec<T>, items_per_page: usize) -> Self {
        Self {
            items,
            current_page: 0,
            items_per_page,
        }
    }

    pub fn total_pages(&self) -> usize {
        if self.items.is_empty() {
            1
        } else {
            (self.items.len() + self.items_per_page - 1) / self.items_per_page
        }
    }

    pub fn can_go_prev(&self) -> bool {
        self.current_page > 0
    }

    pub fn can_go_next(&self) -> bool {
        self.current_page < self.total_pages() - 1
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

    pub fn get_current_page_items(&self) -> &[T] {
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.items.len());
        &self.items[start..end]
    }

    pub fn get_current_page_range(&self) -> (usize, usize) {
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.items.len());
        (start, end)
    }
}

/// Simple pagination state for non-generic use cases
#[derive(Debug, Clone)]
pub struct PaginatedGrid {
    pub current_page: usize,
    pub total_items: usize,
    pub items_per_page: usize,
}

impl PaginatedGrid {
    pub fn new(total_items: usize, items_per_page: usize) -> Self {
        Self {
            current_page: 0,
            total_items,
            items_per_page,
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

    pub fn get_current_page_range(&self) -> (usize, usize) {
        let start = self.current_page * self.items_per_page;
        let end = (start + self.items_per_page).min(self.total_items);
        (start, end)
    }
}
