use std::{
    sync::{Arc, Mutex},
    thread::sleep,
};

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::mpsc::{self, Sender},
    task::JoinHandle,
    time,
};

#[allow(dead_code)]
pub mod color;
pub mod console;
pub mod envelope;
pub mod hue;
pub mod serialize;
#[allow(dead_code)]
pub mod wled;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum Onset {
    Full(f32),
    Atmosphere(f32, u16),
    Note(f32, u16),
    Drum(f32),
    Hihat(f32),
    Raw(f32),
}

#[allow(unused_variables)]
pub trait LightService {
    fn onset_detected(&mut self, event: Onset) {}
    fn process_spectrum(&mut self, freq_bins: &[f32]) {}
    fn update(&mut self) {}
}

impl LightService for [Box<dyn LightService + Send>] {
    fn onset_detected(&mut self, onset: Onset) {
        for service in self {
            service.onset_detected(onset);
        }
    }

    fn process_spectrum(&mut self, freq_bins: &[f32]) {
        for service in self {
            service.process_spectrum(freq_bins);
        }
    }

    fn update(&mut self) {
        for service in self {
            service.update();
        }
    }
}

pub trait Pollable {
    fn poll(&self) -> Bytes;
}

#[async_trait]
pub trait Writeable {
    async fn write_data(&mut self, data: &Bytes) -> std::io::Result<()>;
}

#[async_trait]
impl Writeable for tokio::net::UdpSocket {
    async fn write_data(&mut self, data: &Bytes) -> std::io::Result<()> {
        self.send(data).await?;
        Ok(())
    }
}

#[async_trait]
pub trait Closeable {
    async fn close_connection(&mut self);
}

#[async_trait]
impl Closeable for tokio::net::UdpSocket {
    async fn close_connection(&mut self) {
        // UDP socket does not need to be closed
    }
}

pub trait Stream: Writeable + Closeable {}

impl Stream for tokio::net::UdpSocket {}

#[derive(Debug)]
pub struct PollingHelper {
    pub polling_frequency: u16,
    tx: Sender<()>,
    handle: JoinHandle<()>,
}

type Poll = Arc<Mutex<dyn Pollable + Send + Sync + 'static>>;

impl PollingHelper {
    pub fn init(
        mut stream: impl Stream + Send + Sync + 'static,
        pollable: Poll,
        polling_frequency: u16,
    ) -> PollingHelper {
        let (tx, mut rx) = mpsc::channel(1);

        let handle = tokio::task::spawn(async move {
            select! {
                _ = async {
                    loop {
                        let bytes = { pollable.clone().lock().unwrap().poll() };
                        stream.write_data(&bytes).await.unwrap();

                        time::sleep(std::time::Duration::from_secs_f64(
                            1.0 / polling_frequency as f64,
                        ))
                        .await;
                    }
                } => {}
                _ = rx.recv() => {
                    stream.close_connection().await;
                }
            }
        });

        PollingHelper {
            polling_frequency,
            tx,
            handle,
        }
    }
}

impl Drop for PollingHelper {
    fn drop(&mut self) {
        self.tx.blocking_send(()).unwrap();

        while !self.handle.is_finished() {
            sleep(std::time::Duration::from_millis(10));
        }
    }
}
