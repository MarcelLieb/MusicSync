use std::{
    sync::{Arc, Mutex},
    thread::sleep,
};

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::{
    runtime::Handle,
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

pub trait OnsetConsumer {
    fn onset_detected(&mut self, event: Onset);
    fn update(&mut self);
}

impl OnsetConsumer for [Box<dyn OnsetConsumer + Send>] {
    fn onset_detected(&mut self, onset: Onset) {
        for service in self {
            service.onset_detected(onset);
        }
    }

    fn update(&mut self) {
        for service in self {
            service.update();
        }
    }
}

pub trait SpectrumConsumer {
    fn process_spectrum(&mut self, freq_bins: &[f32]);
}

pub trait SpectrumOnsetConsumer: SpectrumConsumer + OnsetConsumer {}

#[allow(dead_code)]
pub enum LightService {
    Spectral(Box<dyn SpectrumConsumer + Send>),
    Onset(Box<dyn OnsetConsumer + Send>),
    SpectralOnset(Box<dyn SpectrumOnsetConsumer + Send>)
}



impl OnsetConsumer for LightService {
    fn onset_detected(&mut self, event: Onset) {
        match self {
            LightService::Onset(consumer) => consumer.onset_detected(event),
            LightService::SpectralOnset(consumer) => consumer.onset_detected(event),
            LightService::Spectral(_) => {},
        }
    }

    fn update(&mut self) {
        if let LightService::Onset(consumer) = self {
            consumer.update();
        }
    }
}

impl SpectrumConsumer for LightService {
    fn process_spectrum(&mut self, freq_bins: &[f32]) {
        match self {
            LightService::Spectral(consumer) => consumer.process_spectrum(freq_bins),
            LightService::SpectralOnset(consumer) => {
                consumer.process_spectrum(freq_bins)},
            LightService::Onset(_) => {},
        }
    }
}

impl OnsetConsumer for [LightService] {
    fn onset_detected(&mut self, event: Onset) {
        for service in self {
            service.onset_detected(event);
        }
    }

    fn update(&mut self) {
        for service in self {
            service.update()
        }
    }
}

impl SpectrumConsumer for [LightService] {
    fn process_spectrum(&mut self, freq_bins: &[f32]) {
        for service in self {
            service.process_spectrum(freq_bins)
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
    tx: Sender<bool>,
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
        let tx = self.tx.clone();

        if Handle::try_current().is_ok() {
            tokio::spawn(async move {
                tx.send(true).await.unwrap();
            });
        } else {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    tx.send(true).await.unwrap();
                });
        }

        while !self.handle.is_finished() {
            sleep(std::time::Duration::from_millis(10));
        }
    }
}
