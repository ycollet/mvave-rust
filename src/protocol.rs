use std::{collections::VecDeque, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use async_trait::async_trait;
use tokio::time::timeout;

use crate::config::{Config, SIZE};

pub const QUERY: u8 = 0x11;
pub const READ: u8 = 0x23;
pub const WRITE: u8 = 0x22;
const MAX_PAYLOAD: usize = 4096;
const READ_CHUNK: usize = 1000;
const WRITE_CHUNK: usize = 173;

pub fn checksum(payload: &[u8]) -> u8 {
    !payload.iter().fold(0u8, |sum, b| sum.wrapping_add(*b))
}

pub fn frame(command: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0, 0x59, command];
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes()[..3]);
    bytes.extend_from_slice(payload);
    bytes.push(checksum(payload));
    bytes
}

pub fn memory_header(address: usize, length: usize) -> Result<Vec<u8>> {
    ensure!(
        length > 0 && address.checked_add(length).is_some_and(|end| end <= SIZE),
        "Configuration address out of range"
    );
    let mut payload = vec![4];
    payload.extend_from_slice(&u32::try_from(address)?.to_le_bytes());
    payload.extend_from_slice(&u32::try_from(length)?.to_le_bytes()[..3]);
    Ok(payload)
}

#[derive(Debug, PartialEq, Eq)]
pub struct Frame {
    pub command: u8,
    pub payload: Vec<u8>,
}

#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn push(&mut self, data: &[u8]) -> Result<()> {
        ensure!(
            self.buffer.len() + data.len() <= MAX_PAYLOAD * 4,
            "Receive buffer overflow"
        );
        self.buffer.extend_from_slice(data);
        Ok(())
    }

    pub fn next(&mut self) -> Result<Option<Frame>> {
        let Some(start) = self.buffer.windows(2).position(|w| w == [0, 0x59]) else {
            if self.buffer.last() == Some(&0) {
                self.buffer.clear();
                self.buffer.push(0);
            } else {
                self.buffer.clear();
            }
            return Ok(None);
        };
        self.buffer.drain(..start);
        if self.buffer.len() < 6 {
            return Ok(None);
        }
        let len = usize::from(self.buffer[3])
            | usize::from(self.buffer[4]) << 8
            | usize::from(self.buffer[5]) << 16;
        if len > MAX_PAYLOAD {
            self.buffer.clear();
            bail!("Invalid control frame length {len}");
        }
        if self.buffer.len() < len + 7 {
            return Ok(None);
        }
        let bytes = self.buffer.drain(..len + 7).collect::<Vec<_>>();
        ensure!(
            checksum(&bytes[6..6 + len]) == bytes[6 + len],
            "Control frame checksum mismatch"
        );
        Ok(Some(Frame {
            command: bytes[2],
            payload: bytes[6..6 + len].to_vec(),
        }))
    }
}

#[async_trait]
pub trait Link: Send {
    async fn send(&mut self, bytes: &[u8]) -> Result<()>;
    async fn receive(&mut self) -> Result<Vec<u8>>;
    async fn disconnect(&mut self) -> Result<()>;
}

pub struct Client {
    link: Box<dyn Link>,
    decoder: Decoder,
}

impl Client {
    pub fn new(link: Box<dyn Link>) -> Self {
        Self {
            link,
            decoder: Decoder::default(),
        }
    }

    async fn transact(&mut self, command: u8, payload: &[u8], reply: u8) -> Result<Vec<u8>> {
        let operation = async {
            self.link.send(&frame(command, payload)).await?;
            loop {
                if let Some(frame) = self.decoder.next()? {
                    if frame.command == reply {
                        return Ok(frame.payload);
                    }
                    bail!(
                        "Unexpected reply 0x{:02X} (expected 0x{reply:02X})",
                        frame.command
                    );
                }
                let bytes = self.link.receive().await?;
                ensure!(!bytes.is_empty(), "Control notification stream closed");
                self.decoder.push(&bytes)?;
            }
        };
        timeout(Duration::from_secs(8), operation)
            .await
            .context("Timed out waiting for pedal; reconnect before retrying")?
    }

    pub async fn identity(&mut self) -> Result<String> {
        let payload = self.transact(QUERY, &[], QUERY).await?;
        let end = payload
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(payload.len());
        let identity = std::str::from_utf8(&payload[..end]).context("Invalid device identity")?;
        ensure!(
            identity.starts_with("FootCtrlPlus_") || identity.starts_with("GAM-10_"),
            "Unsupported device {identity:?}; expected FootCtrlPlus/GAM-10 (not original FootCtrl)"
        );
        Ok(identity.to_string())
    }

    pub async fn read(&mut self) -> Result<Config> {
        let mut bytes = Vec::with_capacity(SIZE);
        while bytes.len() < SIZE {
            let n = READ_CHUNK.min(SIZE - bytes.len());
            let header = memory_header(bytes.len(), n)?;
            let payload = self.transact(READ, &header, READ).await?;
            ensure!(
                payload.len() == n + 8 && payload[..8] == header,
                "Read reply address, type or length mismatch"
            );
            bytes.extend_from_slice(&payload[8..]);
        }
        Config::from_bytes(bytes)
    }

    pub async fn write_changes(&mut self, desired: &Config, baseline: &Config) -> Result<()> {
        for range in desired.differences(baseline) {
            let mut addr = range.start;
            while addr < range.end {
                let end = (addr + WRITE_CHUNK).min(range.end);
                let mut payload = memory_header(addr, end - addr)?;
                payload.extend_from_slice(&desired.bytes()[addr..end]);
                let ack = self.transact(WRITE, &payload, 0).await?;
                ensure!(
                    ack == [0],
                    "Pedal rejected write at 0x{addr:04X}: {ack:02X?}"
                );
                addr = end;
            }
        }
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.link.disconnect().await
    }
}

pub struct SimulatedLink {
    memory: Config,
    replies: VecDeque<Vec<u8>>,
}

impl SimulatedLink {
    pub fn new() -> Self {
        Self {
            memory: Config::demo(),
            replies: VecDeque::new(),
        }
    }
}

#[async_trait]
impl Link for SimulatedLink {
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let mut decoder = Decoder::default();
        decoder.push(bytes)?;
        let request = decoder.next()?.context("Incomplete simulated request")?;
        let reply = match request.command {
            QUERY => frame(QUERY, b"FootCtrlPlus_012-simulated\0"),
            READ | WRITE => {
                ensure!(request.payload.len() >= 8, "Short memory request");
                let header = &request.payload[..8];
                let addr = u32::from_le_bytes(header[1..5].try_into()?) as usize;
                let n = usize::from(header[5])
                    | usize::from(header[6]) << 8
                    | usize::from(header[7]) << 16;
                ensure!(
                    header[0] == 4 && addr.checked_add(n).is_some_and(|end| end <= SIZE),
                    "Invalid simulated memory request"
                );
                if request.command == READ {
                    let mut payload = header.to_vec();
                    payload.extend_from_slice(&self.memory.bytes()[addr..addr + n]);
                    frame(READ, &payload)
                } else {
                    ensure!(request.payload.len() == n + 8, "Short simulated write");
                    let mut memory = self.memory.bytes().to_vec();
                    memory[addr..addr + n].copy_from_slice(&request.payload[8..]);
                    self.memory = Config::from_bytes(memory)?;
                    frame(0, &[0])
                }
            }
            _ => bail!("Unsupported simulated command"),
        };
        self.replies.extend(reply.chunks(20).map(<[u8]>::to_vec));
        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>> {
        self.replies
            .pop_front()
            .context("No simulated reply pending")
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_and_memory_vectors() -> Result<()> {
        assert_eq!(frame(QUERY, &[]), [0, 0x59, 0x11, 0, 0, 0, 0xff]);
        assert_eq!(
            frame(READ, &memory_header(0, 1000)?),
            [0, 0x59, 0x23, 8, 0, 0, 4, 0, 0, 0, 0, 0xe8, 3, 0, 0x10]
        );
        assert_eq!(checksum(&[255, 1, 5]), 250);
        assert!(memory_header(SIZE - 1, 2).is_err());
        Ok(())
    }

    #[test]
    fn fragmented_coalesced_and_corrupt_frames() -> Result<()> {
        let bytes = frame(QUERY, b"FootCtrlPlus_012");
        let mut decoder = Decoder::default();
        decoder.push(&[0x91, 0, 0x77])?;
        assert!(decoder.next()?.is_none());
        for byte in &bytes[..bytes.len() - 1] {
            decoder.push(&[*byte])?;
            assert!(decoder.next()?.is_none());
        }
        decoder.push(&[bytes[bytes.len() - 1]])?;
        decoder.push(&frame(0, &[0]))?;
        assert_eq!(decoder.next()?.unwrap().payload, b"FootCtrlPlus_012");
        assert_eq!(decoder.next()?.unwrap().command, 0);
        let mut corrupt = bytes.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        decoder.push(&corrupt)?;
        assert!(decoder.next().is_err());
        decoder.push(&[0, 0x59, 0x23, 255, 255, 255])?;
        assert!(decoder.next().is_err());
        Ok(())
    }

    #[tokio::test]
    async fn full_read_diff_write_and_verify() -> Result<()> {
        let mut client = Client::new(Box::new(SimulatedLink::new()));
        assert!(client.identity().await?.contains("simulated"));
        let original = client.read().await?;
        let mut bytes = original.bytes().to_vec();
        bytes[13..513].fill(42);
        bytes[SIZE - 1] = 5;
        let desired = Config::from_bytes(bytes)?;
        client.write_changes(&desired, &original).await?;
        assert_eq!(client.read().await?, desired);
        client.disconnect().await?;
        Ok(())
    }

    struct BadReply(VecDeque<Vec<u8>>);
    #[async_trait]
    impl Link for BadReply {
        async fn send(&mut self, _: &[u8]) -> Result<()> {
            Ok(())
        }
        async fn receive(&mut self) -> Result<Vec<u8>> {
            self.0.pop_front().context("No reply")
        }
        async fn disconnect(&mut self) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn rejects_nack_wrong_address_and_wrong_model() -> Result<()> {
        let mut bytes = Config::demo().bytes().to_vec();
        bytes[0] = 1;
        let changed = Config::from_bytes(bytes)?;
        let mut client = Client::new(Box::new(BadReply(VecDeque::from([frame(0, &[1])]))));
        assert!(
            client
                .write_changes(&changed, &Config::demo())
                .await
                .is_err()
        );
        let mut payload = memory_header(1, 1000)?;
        payload.extend(vec![0; 1000]);
        let mut client = Client::new(Box::new(BadReply(VecDeque::from([frame(READ, &payload)]))));
        assert!(client.read().await.is_err());
        let mut client = Client::new(Box::new(BadReply(VecDeque::from([frame(
            QUERY,
            b"FootCtrl_012\0",
        )]))));
        assert!(client.identity().await.is_err());
        Ok(())
    }
}
