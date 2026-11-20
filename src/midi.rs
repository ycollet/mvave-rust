use anyhow::{Context, Result, ensure};
use midir::{Ignore, MidiInput, MidiInputConnection};
use tokio::sync::mpsc::UnboundedSender;

use crate::{config::hex, device::Event};

pub fn ports() -> Result<Vec<String>> {
    let midi = MidiInput::new("mwave-rust")?;
    midi.ports()
        .iter()
        .map(|port| midi.port_name(port).context("Reading MIDI port name"))
        .collect()
}

pub fn monitor(index: usize, events: UnboundedSender<Event>) -> Result<MidiInputConnection<()>> {
    let mut midi = MidiInput::new("mwave-rust monitor")?;
    midi.ignore(Ignore::None);
    let ports = midi.ports();
    ensure!(
        index < ports.len(),
        "MIDI port {index} does not exist; use --list-midi"
    );
    midi.connect(
        &ports[index],
        "mwave-rust monitor",
        move |timestamp, bytes, _| {
            let _ = events.send(Event::Midi(format!(
                "{:.3}s  {}",
                timestamp as f64 / 1_000_000.0,
                hex(bytes)
            )));
        },
        (),
    )
    .map_err(|error| anyhow::anyhow!("Opening MIDI port: {error}"))
}
