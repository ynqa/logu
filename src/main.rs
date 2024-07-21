use std::io;

use clap::Parser;
use promkit::crossterm::{
    self, cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    style,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{self, Duration},
};
use tokio_util::sync::CancellationToken;

mod drain;
use drain::Drain;
mod stdin;

/// Interactive grep (for streaming)
#[derive(Parser)]
#[command(name = "dlg", version)]
#[command(name = "dlg", version)]
pub struct Args {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;

    let canceler = CancellationToken::new();

    let canceled = canceler.clone();
    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move { stdin::streaming(tx, Duration::from_millis(10), canceled).await });

    let _: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        let draw_interval = time::interval(Duration::from_millis(100));
        let train_interval = time::interval(Duration::from_millis(10));
        futures::pin_mut!(draw_interval);
        futures::pin_mut!(train_interval);

        let mut drain = Drain::default();

        loop {
            tokio::select! {
                _ = train_interval.tick() => {
                    match rx.recv().await {
                        Some(msg) => {
                            drain.train(msg);
                        }
                        None => break,
                    }
                }
                _ = draw_interval.tick() => {
                    let terminal_size = crossterm::terminal::size()?;
                    crossterm::execute!(
                        io::stdout(),
                        terminal::Clear(terminal::ClearType::All),
                        cursor::MoveTo(0, 0),
                    )?;
                    for cluster in drain.clusters().iter().take(terminal_size.1 as usize) {
                        crossterm::execute!(
                            io::stdout(),
                            style::Print(cluster.to_string()),
                            cursor::MoveToNextLine(1),
                        )?;
                    }
                }
            }
        }
        Ok(())
    });

    loop {
        let event = event::read()?;
        if event
            == Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            })
        {
            break;
        }
    }

    canceler.cancel();

    disable_raw_mode()?;
    Ok(())
}
