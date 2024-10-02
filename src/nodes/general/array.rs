use std::{collections::VecDeque, sync::Arc};

use log::{debug, info};
use tokio::{
    select,
    sync::{broadcast, oneshot},
};

use crate::nodes::{internal, NodeTrait, CHANNEL_SIZE};

pub struct Aggregate<I: Clone + Send> {
    sender: broadcast::Sender<Arc<[I]>>,
    receiver: Option<broadcast::Receiver<I>>,
    handle: Option<tokio::task::JoinHandle<VecDeque<I>>>,
    buffer: Option<VecDeque<I>>,
    stop_signal: Option<oneshot::Sender<()>>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send + Sync> internal::Getters<I, Arc<[I]>, VecDeque<I>> for Aggregate<I> {
    fn get_sender(&self) -> &broadcast::Sender<Arc<[I]>> {
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
    pub fn init(size: usize, hop_size: usize) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            sender,
            receiver: None,
            handle: None,
            buffer: None,
            stop_signal: None,
            size,
            hop_size,
        }
    }

    async fn stop_task(&mut self) {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
            if let Some(handle) = self.handle.take() {
                self.buffer.replace(handle.await.unwrap());
            }
        }
    }
}

impl<I: Clone + Send + Sync + 'static> NodeTrait<I, Arc<[I]>, VecDeque<I>> for Aggregate<I> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, I, F>) {
        self.stop_task().await;

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
                                if buffer.len() >= size {
                                    let data = Arc::from(buffer.make_contiguous()[..size].to_vec());
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

    async fn unfollow(&mut self) {
        self.stop_task().await;
    }
}

pub struct Window<I: Clone + Send> {
    sender: broadcast::Sender<Arc<[I]>>,
    receiver: Option<broadcast::Receiver<Arc<[I]>>>,
    handle: Option<tokio::task::JoinHandle<VecDeque<I>>>,
    buffer: Option<VecDeque<I>>,
    stop_signal: Option<oneshot::Sender<()>>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send> Window<I> {
    pub fn init(size: usize, hop_size: usize) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE * (size / hop_size + 1));
        Self {
            sender,
            receiver: None,
            handle: None,
            buffer: None,
            stop_signal: None,
            size,
            hop_size,
        }
    }

    async fn stop_task(&mut self) {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
            if let Some(handle) = self.handle.take() {
                self.buffer.replace(handle.await.unwrap());
            }
        }
    }
}

impl<I: Clone + Send + Sync> internal::Getters<Arc<[I]>, Arc<[I]>, VecDeque<I>> for Window<I> {
    fn get_sender(&self) -> &broadcast::Sender<Arc<[I]>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<Arc<[I]>>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<VecDeque<I>>> {
        &mut self.handle
    }
}

impl<I: Clone + Send + Sync + 'static> NodeTrait<Arc<[I]>, Arc<[I]>, VecDeque<I>> for Window<I> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, Arc<[I]>, F>) {
        self.stop_task().await;

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
                                info!("Data received");
                                buffer.extend(data.iter().cloned());
                                while buffer.len() > size {
                                    let data = Arc::from(buffer.make_contiguous()[..size].to_vec());
                                    let mut status = sender.send(data);
                                    while status.is_err() {
                                        tokio::task::yield_now().await;
                                        status = sender.send(status.err().unwrap().0);
                                    }
                                    buffer.drain(0..hop_size);
                                    tokio::task::yield_now().await;
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

    async fn unfollow(&mut self) {
        self.stop_task().await;
    }
}

pub struct Retimer<I: Clone + Send> {
    sender: broadcast::Sender<I>,
    receiver: Option<broadcast::Receiver<I>>,
    handle: Option<tokio::task::JoinHandle<Option<I>>>,
    stop_signal: Option<oneshot::Sender<()>>,
    interval: std::time::Duration,
    buffer: Option<I>,
}

impl<I: Clone + Send + Sync> internal::Getters<I, I, Option<I>> for Retimer<I> {
    fn get_sender(&self) -> &broadcast::Sender<I> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<I>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<Option<I>>> {
        &mut self.handle
    }
}

impl<I: Clone + Send> Retimer<I> {
    pub fn init(interval: std::time::Duration) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            sender,
            receiver: None,
            handle: None,
            stop_signal: None,
            interval,
            buffer: None,
        }
    }

    pub fn init_hz(hz: f64) -> Self {
        let interval = std::time::Duration::from_secs_f64(1.0 / hz);
        Self::init(interval)
    }

    async fn stop_task(&mut self) {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
            if let Some(handle) = self.handle.take() {
                self.buffer = handle.await.unwrap();
            }
        }
    }
}

impl<I: Clone + Send + Sync + 'static> NodeTrait<I, I, Option<I>> for Retimer<I> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, I, F>) {
        self.stop_task().await;

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        self.stop_signal.replace(stop_tx);

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let mut buffer = self.buffer.take();
        let interval = self.interval;
        let mut interval = tokio::time::interval(interval);

        let handle = tokio::spawn(async move {
            // Make sure the buffer is filled
            // eliminates one if statement in the loop
            if buffer.is_none() {
                match receiver.recv().await {
                    Ok(data) => {
                        buffer.replace(data);
                    }
                    Err(e) => match e {
                        broadcast::error::RecvError::Closed => return buffer,
                        broadcast::error::RecvError::Lagged(n) => {
                            info!("Buffer lagged by {} messages", n);
                            loop {
                                if let Ok(data) = receiver.recv().await {
                                    buffer.replace(data);
                                    break;
                                }
                            }
                        }
                    },
                }
            }
            select! {
                _ = stop_rx => {
                    debug!("Buffer stopped");
                    return buffer;
                }
                _ = async {
                    loop {
                        interval.tick().await;
                        let data = buffer.take().unwrap();
                        let mut status = sender.send(data);
                        while status.is_err() {
                            tokio::task::yield_now().await;
                            status = sender.send(status.err().unwrap().0);
                        }
                        // Wait for data to arrive
                        match receiver.recv().await {
                            Ok(data) => {
                                buffer.replace(data);
                            }
                            Err(e) => match e {
                                broadcast::error::RecvError::Closed => break,
                                broadcast::error::RecvError::Lagged(n) => {
                                    info!("Buffer lagged by {} messages", n);
                                    loop {
                                        if let Ok(data) = receiver.recv().await {
                                            buffer.replace(data);
                                            break;
                                        }
                                    }
                                },
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

    async fn unfollow(&mut self) {
        self.stop_task().await;
    }
}
