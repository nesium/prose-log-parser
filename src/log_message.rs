use std::borrow::Cow;
use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, Utc};
use ratatui::style::Color;
use ratatui::text::Line;
use serde::Deserialize;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};
use syntect_tui::into_span;

use crate::pretty_print::to_writer_pretty;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Fields {
    pub message: String,
    pub direction: Option<StanzaDirection>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum StanzaDirection {
    In,
    Out,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogMessage {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub fields: Fields,
    pub target: String,
    pub span: Option<Span>,
    pub spans: Option<Vec<Span>>,
}

impl FromStr for LogMessage {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(&s)
    }
}

impl LogMessage {
    pub fn pretty_printed_xml(&self) -> Result<String> {
        if self.fields.direction.is_none() {
            return Ok(self.fields.message.to_string());
        }
        let mut buf = Vec::new();
        to_writer_pretty(&mut buf, self.fields.message.as_ref())?;
        Ok(String::from_utf8(buf)?)
    }

    pub fn highlighted_stanza_xml(&self, syntax_set: &SyntaxSet, theme: &Theme) -> Result<String> {
        if self.fields.direction.is_none() {
            return Ok(self.fields.message.to_string());
        }

        let xml = self.pretty_printed_xml()?;

        let mut buf = String::new();
        let syntax = syntax_set
            .find_syntax_by_extension("xml")
            .ok_or(anyhow::format_err!("Missing syntax reference for XML."))?;
        let mut highlighter = HighlightLines::new(syntax, theme);

        for line in LinesWithEndings::from(&xml) {
            let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, &syntax_set)?;
            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
            buf.push_str(&escaped);
        }

        Ok(buf)
    }

    pub fn highlighted_stanza_xml_text(
        &self,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<Vec<Line<'static>>> {
        if self.fields.direction.is_none() {
            let mut lines = vec![];
            for line in LinesWithEndings::from(&self.fields.message) {
                lines.push(Line::styled(
                    line.to_string(),
                    ratatui::style::Style::default().fg(Color::White),
                ));
            }
            return Ok(lines);
        }

        let xml = self.pretty_printed_xml()?;

        let mut lines = Vec::<Line>::new();
        let syntax = syntax_set
            .find_syntax_by_extension("xml")
            .ok_or(anyhow::format_err!("Missing syntax reference for XML."))?;
        let mut highlighter = HighlightLines::new(syntax, theme);

        for line in LinesWithEndings::from(&xml) {
            let line_spans = highlighter
                .highlight_line(line, &syntax_set)?
                .into_iter()
                .map(|segment| {
                    into_span(segment).map(|span| {
                        let mut style = ratatui::style::Style::default();
                        if let Some(fg) = span.style.fg {
                            style = style.fg(fg);
                        }
                        ratatui::text::Span {
                            content: Cow::Owned(span.content.into_owned()),
                            style,
                        }
                    })
                })
                .collect::<Result<Vec<ratatui::text::Span<'static>>, _>>()?;
            lines.push(line_spans.into());
        }

        Ok(lines)
    }
}
