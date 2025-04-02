use ratatui::prelude::{Line, Text};
use ratatui::widgets::ScrollbarState;

pub struct SelectedLogMessage {
    pub message: Text<'static>,
    pub vertical_position: u16,
    pub scroll_state: ScrollbarState,
}

impl From<Vec<Line<'static>>> for SelectedLogMessage {
    fn from(value: Vec<Line<'static>>) -> Self {
        let lines = value.len();

        SelectedLogMessage {
            message: value.into(),
            vertical_position: 0,
            scroll_state: ScrollbarState::default().content_length(lines as u16),
        }
    }
}

impl SelectedLogMessage {
    pub fn prev(&mut self) {
        self.vertical_position = self.vertical_position.saturating_sub(1);
        self.scroll_state = self.scroll_state.position(self.vertical_position);
    }

    pub fn next(&mut self) {
        self.vertical_position = self.vertical_position.saturating_add(1);
        self.scroll_state = self.scroll_state.position(self.vertical_position);
    }

    pub fn scroll_position(&self) -> (u16, u16) {
        (self.vertical_position, 0)
    }
}
