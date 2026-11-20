use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    ble::BleLink,
    config::{Config, validate_lists},
    protocol::{Client, SimulatedLink},
};

pub enum Command {
    Connect,
    Read,
    Write(Config),
    Shutdown,
}

pub enum Event {
    Connected(String, Config),
    Read(Config),
    Written(Config, PathBuf),
    Error(String),
    Midi(String),
}

pub struct Options {
    pub demo: bool,
    pub device: Option<String>,
    pub packet_size: usize,
    pub backup_dir: PathBuf,
}

pub async fn run(
    options: Options,
    mut commands: UnboundedReceiver<Command>,
    events: UnboundedSender<Event>,
) -> Result<()> {
    let mut client: Option<Client> = None;
    let mut baseline: Option<Config> = None;
    let mut keep_alive = tokio::time::interval(Duration::from_secs(10));
    keep_alive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let command = tokio::select! {
            command = commands.recv() => command.unwrap_or(Command::Shutdown),
            _ = keep_alive.tick(), if client.is_some() => {
                if let Some(active) = &mut client
                    && let Err(error) = active.identity().await
                {
                    events.send(Event::Error(format!("Keep-alive failed: {error:#}")))?;
                    close(&mut client, &events).await;
                    baseline = None;
                }
                continue;
            }
        };
        if matches!(command, Command::Shutdown) {
            if let Some(active) = &mut client {
                active.disconnect().await?;
            }
            return Ok(());
        }
        let result: Result<Event> = async {
            match command {
                Command::Connect => {
                    if let Some(active) = &mut client {
                        active.disconnect().await?;
                    }
                    client = None;
                    baseline = None;
                    let link: Box<dyn crate::protocol::Link> = if options.demo {
                        Box::new(SimulatedLink::new())
                    } else {
                        Box::new(
                            BleLink::connect(options.device.as_deref(), options.packet_size)
                                .await?,
                        )
                    };
                    client = Some(Client::new(link));
                    let active = client.as_mut().context("Connection setup failed")?;
                    let identity = active.identity().await?;
                    let config = active.read().await?;
                    baseline = Some(config.clone());
                    Ok(Event::Connected(identity, config))
                }
                Command::Read => {
                    let active = client.as_mut().context("Not connected; press c")?;
                    let config = active.read().await?;
                    baseline = Some(config.clone());
                    Ok(Event::Read(config))
                }
                Command::Write(desired) => {
                    validate_lists(&desired)?;
                    let active = client.as_mut().context("Not connected; press c")?;
                    let original = baseline.as_ref().context("Read the pedal before writing")?;
                    ensure!(
                        active.read().await? == *original,
                        "Pedal changed since last read; read it again before writing"
                    );
                    std::fs::create_dir_all(&options.backup_dir)
                        .context("Creating backup directory")?;
                    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
                    let path = options.backup_dir.join(format!("before-write-{stamp}.fcp"));
                    original
                        .save(&path)
                        .context("Backup failed; no settings written")?;
                    let original = baseline.take().context("Missing write baseline")?;
                    active.write_changes(&desired, &original).await?;
                    let actual = active.read().await?;
                    ensure!(
                        actual == desired,
                        "Readback does not match requested configuration; backup: {}",
                        path.display()
                    );
                    baseline = Some(actual.clone());
                    Ok(Event::Written(actual, path))
                }
                Command::Shutdown => unreachable!(),
            }
        }
        .await;
        match result {
            Ok(event) => {
                events.send(event)?;
            }
            Err(error) => {
                events.send(Event::Error(format!("{error:#}. Session closed; reconnect/read before retrying. A failed write may have partially applied.")))?;
                baseline = None;
                close(&mut client, &events).await;
            }
        }
    }
}

async fn close(client: &mut Option<Client>, events: &UnboundedSender<Event>) {
    if let Some(mut active) = client.take()
        && let Err(error) = active.disconnect().await
    {
        let _ = events.send(Event::Error(format!("Disconnect failed: {error:#}")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    async fn worker_backs_up_before_verified_write() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let (tx, rx) = unbounded_channel();
        let (events, mut output) = unbounded_channel();
        let task = tokio::spawn(run(
            Options {
                demo: true,
                device: None,
                packet_size: 188,
                backup_dir: dir.path().to_path_buf(),
            },
            rx,
            events,
        ));
        tx.send(Command::Connect)?;
        let Event::Connected(_, original) =
            output.recv().await.context("Missing connected event")?
        else {
            anyhow::bail!("Connection failed");
        };
        let mut bytes = original.bytes().to_vec();
        bytes[2] = 15;
        let desired = Config::from_bytes(bytes)?;
        tx.send(Command::Write(desired.clone()))?;
        let Event::Written(actual, backup) = output.recv().await.context("Missing write event")?
        else {
            anyhow::bail!("Write failed");
        };
        assert_eq!(actual, desired);
        assert_eq!(Config::load(&backup)?, original);
        tx.send(Command::Shutdown)?;
        task.await??;
        Ok(())
    }
}
