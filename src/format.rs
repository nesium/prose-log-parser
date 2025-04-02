use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::log_message::{LogMessage, StanzaDirection};

pub async fn read_and_parse_json_lines(path: impl AsRef<Path>, color: bool) -> Result<()> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();
    let theme = &theme_set.themes["base16-ocean.dark"];

    while let Some(line) = lines.next_line().await? {
        let message = LogMessage::from_str(&line)?;

        let direction = match message.fields.direction {
            Some(StanzaDirection::In) => "in",
            Some(StanzaDirection::Out) => "out",
            None => {
                println!("<!--\n{}\n-->\n", message.fields.message);
                continue;
            }
        };

        let formatted_message = if color {
            message.highlighted_stanza_xml(&syntax_set, &theme)?
        } else {
            message.pretty_printed_xml()?
        };

        println!("<!-- {direction} -->\n{formatted_message}\n");
    }

    Ok(())
}
