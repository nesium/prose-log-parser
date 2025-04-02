use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::iter::once;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use clipboard::{ClipboardContext, ClipboardProvider};
use parking_lot::Mutex;
use ratatui::backend::Backend;
use ratatui::layout::{Margin, Rect};
use ratatui::prelude::{Color, Modifier, Span, Style, Stylize, Text};
use ratatui::widgets::{
    Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
};
use ratatui::Frame;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::log_message::{LogMessage, StanzaDirection};
use crate::tui::selected_log_message::SelectedLogMessage;
use crate::tui::stateful_list::StatefulList;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum AppArea {
    Spans,
    #[default]
    Messages,
    MessageDetail,
}

#[derive(Clone)]
pub struct App {
    path: PathBuf,
    inner: Arc<Mutex<AppInner>>,
}

struct AppInner {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    all_messages: StatefulList<LogMessage>,
    messages: StatefulList<LogMessage>,
    spans: StatefulList<String>,
    formatted_message: Option<SelectedLogMessage>,
    selected_area: AppArea,
}

impl App {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            path: path.as_ref().to_path_buf(),
            inner: Arc::new(Mutex::new(AppInner::new(path)?)),
        })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub fn reload_messages(&self) {
        let mut guard = self.inner.lock();
        let inner = &*guard;

        let selected_message_idx = inner.messages.state.selected();
        let is_at_bottom =
            selected_message_idx == Some(inner.messages.items.len().saturating_sub(1));

        let mut updated_state = AppInner::new(&self.path).unwrap();
        updated_state
            .spans
            .state
            .select(inner.spans.state.selected());
        updated_state.selected_area = inner.selected_area.clone();

        *updated_state.messages.state.offset_mut() = inner.messages.state.offset();
        *updated_state.spans.state.offset_mut() = inner.spans.state.offset();

        if is_at_bottom {
            updated_state.messages.select_last();
        } else {
            updated_state.messages.state.select(selected_message_idx);
        }

        updated_state.update_selected_message();

        *guard = updated_state;
    }

    pub fn select_area(&self, area: AppArea) {
        self.inner.lock().selected_area = area
    }

    pub fn handle_key_up(&self) {
        self.inner.lock().handle_key_up()
    }

    pub fn handle_key_down(&self) {
        self.inner.lock().handle_key_down()
    }

    pub fn jump_to_end(&self) {
        let mut state = self.inner.lock();
        state.messages.select_last();
        state.update_selected_message();
    }

    pub fn messages_len(&self) -> usize {
        self.inner.lock().messages.items.len()
    }

    pub fn copy_selected_message_to_clipboard(&self) {
        let Some(message) = self
            .inner
            .lock()
            .messages
            .selected_item()
            .and_then(|m| m.pretty_printed_xml().ok())
        else {
            return;
        };

        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        ctx.set_contents(message).unwrap();
    }
}

impl App {
    pub fn render_selected_message<B: Backend>(&self, f: &mut Frame<B>, rect: Rect) {
        let mut app = self.inner.lock();

        let text = app
            .formatted_message
            .as_ref()
            .map(|m| m.message.clone())
            .unwrap_or(Text::raw("<no selection>"));

        let selected_area = app.selected_area.clone();

        let Some(message) = &mut app.formatted_message else {
            let paragraph = Paragraph::new(text)
                .style(
                    Style::default().fg(if app.selected_area == AppArea::MessageDetail {
                        Color::White
                    } else {
                        Color::DarkGray
                    }),
                )
                .block(Block::default().borders(Borders::ALL).title(Span::styled(
                    "Message Detail (m)",
                    Style::default().add_modifier(Modifier::BOLD),
                )));
            f.render_widget(paragraph, rect);
            return;
        };

        let paragraph = Paragraph::new(text)
            .scroll(message.scroll_position())
            .style(
                Style::default().fg(if selected_area == AppArea::MessageDetail {
                    Color::White
                } else {
                    Color::DarkGray
                }),
            )
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                "Message Detail (m)",
                Style::default().add_modifier(Modifier::BOLD),
            )));

        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("▐");

        f.render_widget(paragraph, rect);
        f.render_stateful_widget(
            scrollbar,
            rect.inner(&Margin {
                vertical: 1,
                horizontal: 0,
            }), // using a inner vertical margin of 1 unit makes the scrollbar inside the block
            &mut message.scroll_state,
        );
    }

    pub fn render_spans_list<B: Backend>(&self, f: &mut Frame<B>, rect: Rect) {
        let mut app = self.inner.lock();

        let span_items = app
            .spans
            .items
            .iter()
            .map(|s| ListItem::new(s.clone()))
            .collect::<Vec<_>>();
        let spans_list = List::new(span_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(if app.selected_area == AppArea::Spans {
                        Color::White
                    } else {
                        Color::DarkGray
                    }))
                    .title(Span::styled(
                        "Spans (s)",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Style::default().bg(Color::LightYellow).fg(Color::Black));

        f.render_stateful_widget(spans_list, rect, &mut app.spans.state);
    }

    pub fn render_messages_list<B: Backend>(&self, f: &mut Frame<B>, rect: Rect) {
        let mut app = self.inner.lock();

        let message_items = app
            .messages
            .items
            .iter()
            .map(|m| {
                let color = match m.fields.direction {
                    None => Color::White,
                    Some(StanzaDirection::In) => Color::Yellow,
                    Some(StanzaDirection::Out) => Color::Blue,
                };
                ListItem::new(m.fields.message.clone()).fg(color)
            })
            .collect::<Vec<_>>();

        let highlight_color = app
            .messages
            .selected_item()
            .and_then(|item| match item.fields.direction {
                None => None,
                Some(StanzaDirection::Out) => Some(Color::Blue),
                Some(StanzaDirection::In) => Some(Color::Yellow),
            })
            .unwrap_or(Color::White);

        // Create a List from all list items and highlight the currently selected one
        let messages_list = List::new(message_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(
                        Style::default().fg(if app.selected_area == AppArea::Messages {
                            Color::White
                        } else {
                            Color::DarkGray
                        }),
                    )
                    .title(Span::styled(
                        "All Messages (a)",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Style::default().bg(highlight_color).fg(Color::Black));

        // We can now render the item list
        f.render_stateful_widget(messages_list, rect, &mut app.messages.state);
    }
}

impl AppInner {
    fn new(path: impl AsRef<Path>) -> Result<Self> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let messages = reader
            .lines()
            .map(|line| {
                line.map_err(anyhow::Error::from)
                    .and_then(|line| line.parse::<LogMessage>().map_err(anyhow::Error::from))
            })
            .collect::<anyhow::Result<Vec<_>, _>>()?;

        let mut spans = messages
            .iter()
            .filter_map(|m| {
                m.spans
                    .as_ref()
                    .map(|spans| spans.iter().map(|s| s.name.clone()).collect::<Vec<_>>())
            })
            .flatten()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        spans.sort();

        let all_messages = StatefulList::with_items(messages);

        Ok(AppInner {
            syntax_set,
            theme_set,
            all_messages: all_messages.clone(),
            messages: all_messages,
            spans: StatefulList::with_items(
                once("[All Messages]".to_string())
                    .chain(spans.into_iter())
                    .collect(),
            ),
            formatted_message: None,
            selected_area: Default::default(),
        })
    }

    fn update_selected_message(&mut self) {
        self.formatted_message = self.messages.selected_item().and_then(|m| {
            m.highlighted_stanza_xml_text(
                &self.syntax_set,
                &self.theme_set.themes["base16-ocean.dark"],
            )
            .ok()
            .map(Into::into)
        })
    }

    fn update_selected_span(&mut self) {
        if self.spans.state.selected() == Some(0) {
            self.messages = self.all_messages.clone();
            return;
        }

        let Some(span_name) = self.spans.selected_item() else {
            self.messages = StatefulList::with_items(vec![]);
            return;
        };

        self.messages = StatefulList::with_items(
            self.all_messages
                .items
                .iter()
                .filter(|m| {
                    m.spans
                        .as_ref()
                        .and_then(|s| s.iter().find(|s| &s.name == span_name))
                        .is_some()
                })
                .cloned()
                .collect(),
        );
    }

    fn handle_key_up(&mut self) {
        match self.selected_area {
            AppArea::Spans => {
                self.spans.prev();
                self.update_selected_span();
            }
            AppArea::Messages => {
                self.messages.prev();
                self.update_selected_message();
            }
            AppArea::MessageDetail => {
                if let Some(m) = &mut self.formatted_message {
                    m.prev()
                }
            }
        }
    }

    fn handle_key_down(&mut self) {
        match self.selected_area {
            AppArea::Spans => {
                self.spans.next();
                self.update_selected_span();
            }
            AppArea::Messages => {
                self.messages.next();
                self.update_selected_message();
            }
            AppArea::MessageDetail => {
                if let Some(m) = &mut self.formatted_message {
                    m.next()
                }
            }
        }
    }
}
