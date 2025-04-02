use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::format::read_and_parse_json_lines;
use crate::tui::browse_log_file;

mod format;
mod log_message;
mod pretty_print;
mod tui;

#[derive(Parser)]
struct LogParser {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Print {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        color: bool,
    },
    Browse {
        #[arg(long)]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match LogParser::parse().cmd {
        Command::Print { path, color } => read_and_parse_json_lines(path, color).await,
        Command::Browse { path } => browse_log_file(path).await,
    }
}
