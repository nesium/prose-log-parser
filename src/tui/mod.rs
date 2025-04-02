use std::io;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{EventStream, KeyEvent};
use crossterm::{
    event::{Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use notify::{Config, PollWatcher, RecursiveMode, Watcher};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use tokio::select;
use tokio::sync::mpsc;

use crate::tui::app::{App, AppArea};

mod app;
mod selected_log_message;
mod stateful_list;

pub async fn browse_log_file(path: impl AsRef<Path>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let path = path.as_ref();
    let app = App::new(path)?;

    let res = run_app(&mut terminal, app).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: App) -> Result<()> {
    let mut reader = EventStream::new();
    let (es_tx, mut es_rx) = mpsc::channel(1);
    let (notify_tx, mut notify_rx) = mpsc::channel::<()>(1);

    // Spawn an async task to listen for key events and send them through the channel
    tokio::spawn(async move {
        while let Some(Ok(event)) = reader.next().await {
            if let Err(_) = es_tx.send(event).await {
                // If we can't send the event, it means the receiver has been dropped, so we should end the loop
                break;
            }
        }
    });

    let mut watcher = PollWatcher::new(
        move |result: Result<notify::Event, notify::Error>| {
            let event = result.unwrap();

            if event.kind.is_modify() {
                notify_tx.blocking_send(()).unwrap();
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(10)),
    )?;
    watcher.watch(app.path(), RecursiveMode::NonRecursive)?;

    terminal.draw(|f| draw_ui(f, &app))?;

    loop {
        select! {
            Some(event) = es_rx.recv() => {
                match event {
                    Event::Key(KeyEvent {
                        code,
                        kind: KeyEventKind::Press,
                        ..
                    }) => {
                        match code {
                            KeyCode::Char('s') => app.select_area(AppArea::Spans),
                            KeyCode::Char('a') => app.select_area(AppArea::Messages),
                            KeyCode::Char('m') => app.select_area(AppArea::MessageDetail),
                            KeyCode::Char('c') => app.copy_selected_message_to_clipboard(),
                            KeyCode::Char('r') => app.reload_messages(),
                            KeyCode::Char('e') => app.jump_to_end(),
                            KeyCode::Up => app.handle_key_up(),
                            KeyCode::Down => app.handle_key_down(),
                            KeyCode::Esc => {
                                // Exit the loop on Esc
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                terminal.draw(|f| draw_ui(f, &app))?;
            },
            Some(_) = notify_rx.recv() => {
                app.reload_messages();
                terminal.draw(|f| draw_ui(f, &app))?;
            },
            else => {
                println!("All channels have been closed");
                break;
            },
        }
    }

    Ok(())
}

fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    // Create two chunks with equal horizontal screen space
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(f.size());

    draw_sidebar(f, app, chunks[0]);
    draw_selected_message(f, app, chunks[1]);
}

fn draw_selected_message<B: Backend>(f: &mut Frame<B>, app: &App, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Max(2)])
        .split(rect);

    app.render_selected_message(f, chunks[0]);

    let legend = vec![(Color::Blue, "Sent"), (Color::Yellow, "Received")];

    let keys = vec![
        ("Esc", "Quit"),
        ("c", "Copy message"),
        ("r", "Reload messages"),
        ("e", "Jump to end"),
    ];

    let mut spans = vec![];

    spans.push(Span::raw(format!("{} messages", app.messages_len())));
    spans.push(Span::raw(" |  "));

    spans.extend(
        legend
            .into_iter()
            .flat_map(|(color, title)| {
                let key = Span::styled("  ", Style::new().fg(Color::Black).bg(color));
                let desc = Span::styled(format!(" {} ", title), Style::new().fg(Color::Gray));
                [key, desc]
            })
            .collect::<Vec<_>>(),
    );

    spans.push(Span::raw(" |  "));

    spans.extend(
        keys.iter()
            .flat_map(|(key, desc)| {
                let key = Span::styled(
                    format!(" {} ", key),
                    Style::new().fg(Color::Black).bg(Color::Gray),
                );
                let desc = Span::styled(format!(" {} ", desc), Style::new().fg(Color::Gray));
                [key, desc]
            })
            .collect::<Vec<_>>(),
    );

    let bottom_bar = Paragraph::new(Line::from(spans));
    f.render_widget(
        bottom_bar,
        chunks[1].inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }),
    );
}

fn draw_sidebar<B: Backend>(f: &mut Frame<B>, app: &App, rect: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(rect);

    app.render_spans_list(f, chunks[0]);
    app.render_messages_list(f, chunks[1]);
}
