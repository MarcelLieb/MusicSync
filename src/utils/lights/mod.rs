use std::{
    sync::{Arc, Mutex},
    thread::sleep,
};

use bytes::Bytes;
use log::{info, trace};
use tokio::{
    select,
    sync::oneshot::{self, Sender},
    task::JoinHandle,
    time,
};

use super::audioprocessing::Onset;

#[allow(dead_code)]
pub mod color;
pub mod console;
pub mod envelope;
#[allow(dead_code)]
pub mod hue;
pub mod serialize;
#[allow(dead_code)]
pub mod wled;

#[allow(unused_variables)]
pub trait LightService {
    fn process_onset(&mut self, event: Onset) {}
    fn process_onsets(&mut self, onsets: &[Onset]) {
        for onset in onsets {
            self.process_onset(*onset)
        }
    }
    fn process_spectrum(&mut self, freq_bins: &[f32]) {}
    fn process_samples(&mut self, samples: &[f32]) {}
    fn update(&mut self) {}
}

impl LightService for [Box<dyn LightService + Send>] {
    fn process_onset(&mut self, onset: Onset) {
        for service in self {
            service.process_onset(onset);
        }
    }

    fn process_spectrum(&mut self, freq_bins: &[f32]) {
        for service in self {
            service.process_spectrum(freq_bins);
        }
    }

    fn process_samples(&mut self, samples: &[f32]) {
        for service in self {
            service.process_samples(samples);
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

pub trait Writeable {
    fn write_data(
        &mut self,
        data: &Bytes,
    ) -> impl std::future::Future<Output = std::io::Result<()>> + Send;
}

impl Writeable for tokio::net::UdpSocket {
    async fn write_data(&mut self, data: &Bytes) -> std::io::Result<()> {
        self.send(data).await?;
        Ok(())
    }
}

pub trait Closeable {
    fn close_connection(&mut self) -> impl std::future::Future<Output = ()> + Send;
}

impl Closeable for tokio::net::UdpSocket {
    async fn close_connection(&mut self) {
        // UDP socket does not need to be closed
    }
}

pub trait Stream: Writeable + Closeable {}

impl Stream for tokio::net::UdpSocket {}

#[derive(Debug)]
pub struct PollingHelper {
    tx: Option<Sender<()>>,
    handle: JoinHandle<()>,
}

type Poll = Arc<Mutex<dyn Pollable + Send + Sync + 'static>>;

impl PollingHelper {
    pub fn init(
        mut stream: impl Stream + Send + Sync + 'static,
        pollable: Poll,
        polling_frequency: f64,
    ) -> PollingHelper {
        let (tx, rx) = oneshot::channel();
        let mut interval =
            time::interval(std::time::Duration::from_secs_f64(1.0 / polling_frequency));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

        let handle = tokio::task::spawn(async move {
            select! {
                _ = async {
                    interval.tick().await;
                    loop {
                        let bytes = { pollable.clone().lock().unwrap().poll() };
                        stream.write_data(&bytes).await.unwrap();

                        interval.tick().await;
                    }
                } => {
                    eprintln!("Never ending loop returned");
                }
                _ = rx => {
                    stream.close_connection().await;
                }
            }
        });

        PollingHelper { tx: Some(tx), handle }
    }
}

impl Drop for PollingHelper {
    fn drop(&mut self) {
        info!("Shutting done background poller");
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
        while !self.handle.is_finished() {
            sleep(std::time::Duration::from_nanos(1));
        }
        trace!("Background poller shut down");
    }
}
