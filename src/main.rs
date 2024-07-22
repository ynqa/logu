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
pub struct Args {
    #[arg(
        long = "retrieval-timeout",
        default_value = "10",
        help = "Timeout to read a next line from the stream in milliseconds."
    )]
    pub retrieval_timeout_millis: u64,

    #[arg(
        long = "render-interval",
        default_value = "100",
        help = "Interval to render the list in milliseconds.",
        long_help = "Adjust this value to prevent screen flickering
        when a large volume of list is rendered in a short period."
    )]
    pub render_interval_millis: u64,

    #[arg(long = "train-interval", default_value = "10")]
    pub train_interval_millis: u64,

    // Drain related params
    #[arg(
        long = "max-clusters",
        default_value = None,
    )]
    pub max_clusters: Option<usize>,
    #[arg(long = "max-node-depth", default_value = "2")]
    pub max_node_depth: usize,
    #[arg(long = "sim-th", default_value = "0.4")]
    pub sim_th: f32,
    #[arg(long = "max-children", default_value = "100")]
    pub max_children: usize,
    #[arg(long = "param-str", default_value = "<*>")]
    pub param_str: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    enable_raw_mode()?;

    let canceler = CancellationToken::new();

    let canceled = canceler.clone();
    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        stdin::streaming(
            tx,
            Duration::from_millis(args.retrieval_timeout_millis),
            canceled,
        )
        .await
    });

    let draining: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
        let render_interval = time::interval(Duration::from_millis(args.render_interval_millis));
        let train_interval = time::interval(Duration::from_millis(args.train_interval_millis));
        futures::pin_mut!(render_interval);
        futures::pin_mut!(train_interval);

        let mut drain = Drain::new(
            args.max_clusters,
            args.max_node_depth,
            args.sim_th,
            args.max_children,
            args.param_str,
        )?;

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
                _ = render_interval.tick() => {
                    let terminal_size = crossterm::terminal::size()?;
                    crossterm::execute!(
                        io::stdout(),
                        terminal::Clear(terminal::ClearType::All),
                        cursor::MoveTo(0, 0),
                    )?;
                    for cluster in drain.clusters().iter().take(terminal_size.1 as usize) {
                        crossterm::execute!(
                            io::stdout(),
                            style::Print(cluster),
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
    draining.await??;

    disable_raw_mode()?;
    Ok(())
}
