use std::{pin::Pin, time::Duration};

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use btleplug::{
    api::{
        Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter,
        ValueNotification, WriteType,
    },
    platform::{Manager, Peripheral},
};
use futures::{Stream, StreamExt};
use uuid::Uuid;

use crate::protocol::Link;

const SERVICE: Uuid = Uuid::from_u128(0x0000ae40_0000_1000_8000_00805f9b34fb);
const WRITE: Uuid = Uuid::from_u128(0x0000ae41_0000_1000_8000_00805f9b34fb);
const NOTIFY: Uuid = Uuid::from_u128(0x0000ae42_0000_1000_8000_00805f9b34fb);

pub struct BleLink {
    peripheral: Peripheral,
    writer: Characteristic,
    notifications: Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    write_type: WriteType,
    packet_size: usize,
}

impl BleLink {
    pub async fn connect(name_filter: Option<&str>, packet_size: usize) -> Result<Self> {
        let manager = Manager::new()
            .await
            .context("Starting Bluetooth; grant your terminal Bluetooth permission")?;
        let adapters = manager.adapters().await?;
        ensure!(!adapters.is_empty(), "No Bluetooth adapter found");
        let mut candidates = Vec::new();
        for adapter in adapters {
            adapter.start_scan(ScanFilter::default()).await?;
            tokio::time::sleep(Duration::from_secs(5)).await;
            let peripherals = adapter.peripherals().await;
            let stop = adapter.stop_scan().await;
            let peripherals = peripherals?;
            stop?;
            for peripheral in peripherals {
                if let Some(properties) = peripheral.properties().await? {
                    let name = properties.local_name.unwrap_or_default();
                    let known = name.starts_with("FootCtrlPlus")
                        || name.starts_with("GAM-10")
                        || properties.services.contains(&SERVICE);
                    if known && name_filter.is_none_or(|filter| name.contains(filter)) {
                        candidates.push(peripheral);
                    }
                }
            }
        }
        ensure!(
            !candidates.is_empty(),
            "No Chocolate Plus found; enable Bluetooth, close CubeSuite, and retry with c"
        );
        ensure!(
            candidates.len() == 1,
            "Multiple pedals found; use --device with a unique advertised name"
        );
        let peripheral = candidates.remove(0);
        peripheral.connect().await.context("Connecting to pedal")?;
        let result = async {
            peripheral.discover_services().await?;
            let characteristics = peripheral.characteristics();
            let writer = characteristics
                .iter()
                .find(|c| c.uuid == WRITE && c.service_uuid == SERVICE)
                .context("Missing Sinco ae41 control characteristic")?
                .clone();
            let notify = characteristics
                .iter()
                .find(|c| c.uuid == NOTIFY && c.service_uuid == SERVICE)
                .context("Missing Sinco ae42 notification characteristic")?;
            let write_type = if writer.properties.contains(CharPropFlags::WRITE) {
                WriteType::WithResponse
            } else {
                ensure!(
                    writer
                        .properties
                        .contains(CharPropFlags::WRITE_WITHOUT_RESPONSE),
                    "Control characteristic is not writable"
                );
                WriteType::WithoutResponse
            };
            let notifications = peripheral.notifications().await?;
            peripheral.subscribe(notify).await?;
            Ok((writer, notifications, write_type))
        }
        .await;
        match result {
            Ok((writer, notifications, write_type)) => Ok(Self {
                peripheral,
                writer,
                notifications,
                write_type,
                packet_size,
            }),
            Err(error) => {
                peripheral
                    .disconnect()
                    .await
                    .context(format!("Disconnecting after setup failure: {error:#}"))?;
                Err(error)
            }
        }
    }
}

#[async_trait]
impl Link for BleLink {
    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        // Packet splitting is opt-in: support for fragmented writes is unverified.
        for chunk in bytes.chunks(self.packet_size) {
            self.peripheral
                .write(&self.writer, chunk, self.write_type)
                .await
                .context("Writing Sinco control packet")?;
            if self.write_type == WriteType::WithoutResponse {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        Ok(())
    }

    async fn receive(&mut self) -> Result<Vec<u8>> {
        while let Some(notification) = self.notifications.next().await {
            if notification.uuid == NOTIFY {
                return Ok(notification.value);
            }
        }
        anyhow::bail!("Bluetooth notification stream closed; reconnect with c")
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.peripheral
            .disconnect()
            .await
            .context("Disconnecting Bluetooth")
    }
}
