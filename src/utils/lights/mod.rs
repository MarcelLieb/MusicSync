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
    time, runtime::Handle,
};

pub mod color;
pub mod console;
pub mod envelope;
pub mod hue;
pub mod serialize;

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    Full(f32),
    Atmosphere(f32, u16),
    Note(f32, u16),
    Drum(f32),
    Hihat(f32),
    Raw(f32),
}

pub trait LightService {
    fn event_detected(&mut self, event: Event);
    fn update(&mut self);
}

pub trait Pollable {
    fn poll(&self) -> Bytes;
}

#[async_trait]
pub trait Writeable {
    async fn write_data(&mut self, data: &Bytes) -> std::io::Result<()>;
}

#[async_trait]
pub trait Closeable {
    async fn close_connection(&mut self);
}

pub trait Stream: Writeable + Closeable {}

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

        if let Ok(_) = Handle::try_current() {
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