use ratatui::widgets::ListState;

#[derive(Debug, Clone)]
pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        let mut list = StatefulList {
            state: ListState::default(),
            items,
        };
        if !list.items.is_empty() {
            list.state.select(Some(0))
        }
        list
    }

    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let idx = self
            .state
            .selected()
            .map(|i| if i < self.items.len() - 1 { i + 1 } else { 0 })
            .unwrap_or(0);
        self.state.select(Some(idx));
    }

    pub fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }

        let idx = self
            .state
            .selected()
            .map(|i| if i > 0 { i - 1 } else { self.items.len() - 1 })
            .unwrap_or(0);
        self.state.select(Some(idx));
    }

    pub fn selected_item(&self) -> Option<&T> {
        if self.items.is_empty() {
            return None;
        }

        let Some(i) = self.state.selected() else {
            return None;
        };

        if i > self.items.len() - 1 {
            return None;
        }

        Some(&self.items[i])
    }

    pub fn select_last(&mut self) {
        self.state.select(Some(self.items.len().saturating_sub(1)));
    }
}
