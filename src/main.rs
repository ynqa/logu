use std::io;

use clap::Parser;
use promkit::{
    crossterm::{
        self, cursor,
        event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers},
        style,
        terminal::{disable_raw_mode, enable_raw_mode},
    },
    grapheme::StyledGraphemes,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    task::JoinHandle,
    time::{self, timeout, Duration},
};
use tokio_util::sync::CancellationToken;

mod drain;
use drain::Drain;

#[derive(Parser)]
#[command(name = "logu", version)]
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

    #[arg(
        long = "cluster-size-th",
        default_value = "0",
        help = "Threshold to filter out small clusters."
    )]
    pub cluster_size_th: usize,

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
    // Avoid the rendering messy by disabling mouse scroll and fixing the row.
    crossterm::execute!(
        io::stdout(),
        crossterm::event::EnableMouseCapture,
        crossterm::cursor::Hide
    )?;

    let canceler = CancellationToken::new();

    let canceled = canceler.clone();
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

        let mut reader = BufReader::new(tokio::io::stdin()).lines();

        while !canceled.is_cancelled() {
            tokio::select! {
                _ = train_interval.tick() => {
                    // Set a timeout to ensure non-blocking behavior,
                    // especially responsive to user inputs like ctrl+c.
                    // Continuously retry until cancellation to prevent loss of logs.
                    let ret = timeout(Duration::from_millis(args.retrieval_timeout_millis), reader.next_line()).await;
                    if ret.is_err() {
                        continue;
                    }

                    let ret = ret?;

                    match ret {
                        Ok(Some(line)) => {
                            let escaped = strip_ansi_escapes::strip_str(line.replace(['\n', '\t'], " "));
                            drain.train(escaped);
                        }
                        _ => break,
                    }
                }
                _ = render_interval.tick() => {
                    let terminal_size = crossterm::terminal::size()?;
                    crossterm::execute!(
                        io::stdout(),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
                        crossterm::terminal::Clear(crossterm::terminal::ClearType::Purge),
                        cursor::MoveTo(0, 0),
                    )?;
                    let mut total_rows = 0;
                    for cluster in drain.clusters().iter()
                        .filter(|cluster| cluster.size > args.cluster_size_th)
                        .take(terminal_size.1 as usize) {
                        let styled = StyledGraphemes::from(cluster.to_string());
                        let rows = styled.matrixify(terminal_size.0 as usize, terminal_size.1 as usize, 0).0;

                        if total_rows + rows.len() > terminal_size.1 as usize {
                            break;
                        }

                        crossterm::execute!(
                            io::stdout(),
                            style::Print(cluster),
                            cursor::MoveToNextLine(1),
                        )?;

                        total_rows += rows.len();
                    }
                }
            }
        }
        Ok(())
    });

    loop {
        let event = crossterm::event::read()?;
        #[allow(clippy::single_match)]
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            }) => {
                break;
            }
            _ => {}
        }
    }

    canceler.cancel();
    draining.await??;

    disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::Show
    )?;
    Ok(())
}
