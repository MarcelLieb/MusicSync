use std::sync::Arc;

use log::info;
use tokio::sync::{broadcast, oneshot};

use crate::{nodes::{internal::Getters, Node, CHANNEL_SIZE}, utils::audioprocessing::MelFilterBank};


struct MelFilterBankNode <I: Clone + Sync> {
    sender: broadcast::Sender<Arc<[I]>>,
    receiver: Option<broadcast::Receiver<Arc<[I]>>>,
    handle: Option<tokio::task::JoinHandle<()>>,
    stop_signal: Option<oneshot::Sender<()>>,
    filter_bank: MelFilterBank,
}

impl <I: Clone + Send + Sync> Getters<Arc<[I]>, Arc<[I]>, ()> for MelFilterBankNode<I> {
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

impl Node<Arc<[f32]>, Arc<[f32]>, ()> for MelFilterBankNode<f32> {
    fn follow<T: Clone + Send, F>(&mut self, node: impl Node<T, Arc<[f32]>, F>) {
        self.unfollow();

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
                                let data = filter_bank.filter_alloc(&data);
                                sender.send(Arc::from(data)).unwrap();
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
    pub fn new(bands: usize, n_fft: u32, sample_rate: u32, high_freq: u32) -> Self {
        let filter_bank = MelFilterBank::init(sample_rate, n_fft, bands, high_freq);
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