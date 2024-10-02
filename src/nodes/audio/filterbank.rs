use std::sync::Arc;

use log::{info, warn};
use tokio::sync::{broadcast, oneshot};

use crate::{
    nodes::{internal::Getters, NodeTrait, CHANNEL_SIZE},
    utils::audioprocessing::MelFilterBank,
};

pub struct MelFilterBankNode<I: Clone + Sync> {
    sender: broadcast::Sender<Arc<[I]>>,
    receiver: Option<broadcast::Receiver<Arc<[I]>>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    stop_signal: Option<oneshot::Sender<()>>,
    filter_bank: MelFilterBank,
}

impl<I: Clone + Send + Sync> Getters<Arc<[I]>, Arc<[I]>, ()> for MelFilterBankNode<I> {
    fn get_sender(&self) -> &broadcast::Sender<Arc<[I]>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<Arc<[I]>>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.handle
    }
}

impl NodeTrait<Arc<[f32]>, Arc<[f32]>, ()> for MelFilterBankNode<f32> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, Arc<[f32]>, F>) {
        self.unfollow().await;

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_signal.replace(stop_tx);

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let filter_bank = self.filter_bank.clone();

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = stop_rx => {},
                _ = async {
                    loop {
                        match receiver.recv().await {
                            Ok(data) => {
                                if data.len() != filter_bank.fft_size as usize {
                                    warn!("Data length does not match FFT size. Skipping.");
                                    continue;
                                }
                                let data = filter_bank.filter_alloc(&data);
                                let mut status = sender.send(data.into());
                                while status.is_err() {
                                    tokio::task::yield_now().await;
                                    status = sender.send(status.err().unwrap().0);
                                }
                            },
                            Err(e) => match e {
                                broadcast::error::RecvError::Closed => break,
                                broadcast::error::RecvError::Lagged(n) => info!("Lagged: {}", n),
                            },
                        }
                    }
                } => {},
            }
        });

        self.handle.replace(handle);
    }
}

impl MelFilterBankNode<f32> {
    pub fn new(
        bands: usize,
        n_fft: u32,
        sample_rate: u32,
        min_frequency: f32,
        max_frequency: f32,
    ) -> Self {
        let filter_bank =
            MelFilterBank::init(sample_rate, n_fft, bands, min_frequency, max_frequency);
        let (sender, _) = broadcast::channel::<Arc<[f32]>>(CHANNEL_SIZE);

        Self {
            sender,
            receiver: None,
            handle: None,
            stop_signal: None,
            filter_bank,
        }
    }
}