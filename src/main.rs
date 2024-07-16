use clap::Parser;
use promkit::crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use tokio::{
    sync::mpsc,
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

    let draining = tokio::spawn(async move {
        let interval = time::interval(Duration::from_micros(10));
        futures::pin_mut!(interval);
        let mut drain = Drain::default();

        loop {
            let _ = interval.tick().await;
            match rx.recv().await {
                Some(msg) => {
                    drain.train(msg);
                }
                None => break,
            }
        }
        drain
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

    disable_raw_mode()?;

    canceler.cancel();
    for cluster in draining.await?.clusters() {
        // crossterm::execute!(
        //     io::stdout(),
        //     style::Print(cluster.to_string()),
        // )?;
        println!("{} {}", cluster.size, cluster.to_string());
    }

    Ok(())
}
