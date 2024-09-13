use std::{collections::VecDeque, sync::Arc};

use log::{debug, info};
use tokio::{
    select,
    sync::{broadcast, oneshot},
};

use crate::nodes::{internal, Node};

struct Aggregate<I: Clone + Send> {
    sender: broadcast::Sender<Arc<Box<[I]>>>,
    receiver: Option<broadcast::Receiver<I>>,
    handle: Option<tokio::task::JoinHandle<VecDeque<I>>>,
    buffer: Option<VecDeque<I>>,
    stop_signal: Option<oneshot::Sender<()>>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send + Sync> internal::Getters<I, Arc<Box<[I]>>, VecDeque<I>> for Aggregate<I> {
    fn get_sender(&self) -> &broadcast::Sender<Arc<Box<[I]>>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<I>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<VecDeque<I>>> {
        &mut self.handle
    }
}

impl<I: Clone + Send> Aggregate<I> {
    fn stop_task(&mut self) {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
            if let Some(handle) = self.handle.take() {
                let rt = tokio::runtime::Handle::current();

                let buffer = rt.block_on(handle).expect(
                    "I don't know if this is possible, if you see this message, please let me know",
                );
                self.buffer.replace(buffer);
            }
        }
    }
}

impl<I: Clone + Send + Sync + 'static> Node<I, Arc<Box<[I]>>, VecDeque<I>> for Aggregate<I> {
    fn follow<T: Clone + Send, F>(&mut self, node: impl Node<T, I, F>) {
        self.stop_task();

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_signal.replace(stop_tx);

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let mut buffer = if self.buffer.is_none() {
            VecDeque::new()
        } else {
            self.buffer.take().unwrap()
        };
        let size = self.size;
        let hop_size = self.hop_size;

        let handle = tokio::spawn(async move {
            select! {
                _ = stop_rx => {
                    debug!("Buffer stopped");
                    return buffer;
                }
                _ = async {
                    loop {
                        match receiver.recv().await {
                            Ok(data) => {
                                buffer.push_back(data);
                                if buffer.len() > size {
                                    let data = Arc::new(buffer.make_contiguous()[..size].to_vec().into_boxed_slice());
                                    let mut status = sender.send(data);
                                    while status.is_err() {
                                        tokio::task::yield_now().await;
                                        status = sender.send(status.err().unwrap().0);
                                    }
                                    buffer.drain(0..hop_size);
                                }
                            }
                            Err(e) => match e {
                                broadcast::error::RecvError::Closed => break,
                                broadcast::error::RecvError::Lagged(n) => info!("Buffer lagged by {} messages", n),
                            },
                        }
                    }
                } => {
                    buffer
                }
            }
        });

        self.handle = Some(handle);
    }

    fn unfollow(&mut self) {
        self.stop_task();
    }
}

struct Window<I: Clone + Send> {
    sender: broadcast::Sender<Arc<Box<[I]>>>,
    receiver: Option<broadcast::Receiver<Arc<Box<[I]>>>>,
    handle: Option<tokio::task::JoinHandle<VecDeque<I>>>,
    buffer: Option<VecDeque<I>>,
    stop_signal: Option<oneshot::Sender<()>>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send> Window<I> {
    fn stop_task(&mut self) {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
            if let Some(handle) = self.handle.take() {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("Could not create runtime");

                let buffer = rt.block_on(handle).expect(
                    "I don't know if this is possible, if you see this message, please let me know",
                );
                self.buffer.replace(buffer);
            }
        }
    }
}

impl<I: Clone + Send + Sync> internal::Getters<Arc<Box<[I]>>, Arc<Box<[I]>>, VecDeque<I>> for Window<I> {
    fn get_sender(&self) -> &broadcast::Sender<Arc<Box<[I]>>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<Arc<Box<[I]>>>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<VecDeque<I>>> {
        &mut self.handle
    }
}

impl <I: Clone + Send + Sync + 'static> Node<Arc<Box<[I]>>, Arc<Box<[I]>>, VecDeque<I>> for Window<I> {
    fn follow<T: Clone + Send, F>(&mut self, node: impl Node<T, Arc<Box<[I]>>, F>) {
        self.stop_task();

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_signal.replace(stop_tx);

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let mut buffer = if self.buffer.is_none() {
            VecDeque::new()
        } else {
            self.buffer.take().unwrap()
        };
        let size = self.size;
        let hop_size = self.hop_size;

        let handle = tokio::spawn(async move {
            select! {
                _ = stop_rx => {
                    debug!("Buffer stopped");
                    return buffer;
                }
                _ = async {
                    loop {
                        match receiver.recv().await {
                            Ok(data) => {
                                buffer.extend(data.iter().cloned());
                                if buffer.len() > size {
                                    let data = Arc::new(buffer.make_contiguous()[..size].to_vec().into_boxed_slice());
                                    let mut status = sender.send(data);
                                    while status.is_err() {
                                        tokio::task::yield_now().await;
                                        status = sender.send(status.err().unwrap().0);
                                    }
                                    buffer.drain(0..hop_size);
                                }
                            }
                            Err(e) => match e {
                                broadcast::error::RecvError::Closed => break,
                                broadcast::error::RecvError::Lagged(n) => info!("Buffer lagged by {} messages", n),
                            },
                        }
                    }
                } => {
                    buffer
                }
            }
        });

        self.handle = Some(handle);
    }
}
