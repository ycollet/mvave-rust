mod app;
mod ble;
mod config;
mod device;
mod midi;
mod protocol;

use std::{io::IsTerminal, path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use crossterm::event::{self, Event as TerminalEvent, KeyEventKind};
use tokio::sync::mpsc::unbounded_channel;

#[derive(Parser)]
#[command(
    version,
    about = "Chocolate Plus native Bluetooth configuration TUI (not original Chocolate)"
)]
struct Args {
    /// Simulate a pedal without Bluetooth or hardware
    #[arg(long)]
    demo: bool,
    /// Open a CubeSuite-compatible .fcp backup, without automatically connecting
    #[arg(long)]
    file: Option<PathBuf>,
    /// Filter Bluetooth pedals by advertised name
    #[arg(long)]
    device: Option<String>,
    /// BLE write packet size; 20 enables experimental fragmentation on small-MTU links
    #[arg(long, default_value_t = 188, value_parser = clap::value_parser!(u16).range(20..=188))]
    ble_packet_size: u16,
    /// Directory for mandatory backups taken before every device write
    #[arg(long, default_value = "backups")]
    backup_dir: PathBuf,
    /// List native MIDI input ports and exit
    #[arg(long)]
    list_midi: bool,
    /// Monitor this native MIDI input port index (see --list-midi)
    #[arg(long)]
    midi_port: Option<usize>,
    /// Validate a .fcp file's known settings and MIDI lists without hardware
    #[arg(long)]
    check: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_midi {
        for (index, name) in midi::ports()?.iter().enumerate() {
            println!("{index}: {name}");
        }
        return Ok(());
    }
    if let Some(path) = args.check {
        let config = config::Config::load(&path)?;
        config::validate_lists(&config)?;
        println!(
            "{}: {} bytes, mode {} ({})",
            path.display(),
            config::SIZE,
            config.get(0),
            config::MODES
                .get(usize::from(config.get(0)))
                .unwrap_or(&"Unknown")
        );
        return Ok(());
    }
    ensure!(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "The TUI requires a terminal; use --help or --check FILE"
    );
    let config = args.file.as_deref().map(config::Config::load).transpose()?;
    let auto_connect = args.demo && config.is_none();
    let (commands, command_rx) = unbounded_channel();
    let (events, mut event_rx) = unbounded_channel();
    let monitor = args
        .midi_port
        .map(|index| midi::monitor(index, events.clone()))
        .transpose()?;
    let worker = tokio::spawn(device::run(
        device::Options {
            demo: args.demo,
            device: args.device,
            packet_size: usize::from(args.ble_packet_size),
            backup_dir: args.backup_dir,
        },
        command_rx,
        events,
    ));
    let mut app = app::App::new(config, commands.clone());
    let mut terminal = ratatui::try_init().context("Entering terminal mode")?;
    let result = (|| -> Result<()> {
        if auto_connect {
            app.start()?;
        }
        while !app.quit {
            while let Ok(event) = event_rx.try_recv() {
                app.event(event);
            }
            terminal.draw(|frame| app.draw(frame))?;
            if event::poll(Duration::from_millis(50))?
                && let TerminalEvent::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.key(key)?;
            }
        }
        Ok(())
    })();
    ratatui::restore();
    drop(monitor);
    commands
        .send(device::Command::Shutdown)
        .context("Device worker stopped before shutdown")?;
    worker.await.context("Device worker panicked")??;
    result
}
