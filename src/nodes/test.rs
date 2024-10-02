use std::{fmt::Debug, sync::Arc, time::Duration};

use dashmap::DashMap;
use tokio::sync::broadcast;

use super::{
    audio::filterbank::MelFilterBankNode,
    general::array::Window,
    internal::Getters,
    Node, NodeTypes, CHANNEL_SIZE,
};

// A Node that sends 0.0 as fast as it can
pub struct ZeroNode {
    sender: broadcast::Sender<f32>,
    receiver: Option<broadcast::Receiver<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Getters<(), f32, ()> for ZeroNode {
    fn get_sender(&self) -> &broadcast::Sender<f32> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<()>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.handle
    }
}

impl Node<(), f32, ()> for ZeroNode {
    async fn follow<T: Clone + Send, F>(&mut self, _: &impl Node<T, (), F>) {}
}

impl ZeroNode {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        let _sender = sender.clone();

        let handle = tokio::spawn(async move {
            loop {
                let _ = _sender.send(0.0);
                tokio::task::yield_now().await;
            }
        });
        Self {
            sender,
            receiver: None,
            handle: Some(handle),
        }
    }
}

// A Node that sends Arc<[0.0_f32]>
pub struct ArrayNode {
    sender: broadcast::Sender<Arc<[f32]>>,
    receiver: Option<broadcast::Receiver<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Getters<(), Arc<[f32]>, ()> for ArrayNode {
    fn get_sender(&self) -> &broadcast::Sender<Arc<[f32]>> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<()>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.handle
    }
}

impl Node<(), Arc<[f32]>, ()> for ArrayNode {
    async fn follow<T: Clone + Send, F>(&mut self, _: &impl Node<T, (), F>) {}
}

impl ArrayNode {
    pub fn new(cooldown: Duration, size: usize) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        let _sender = sender.clone();

        let handle = tokio::spawn(async move {
            loop {
                let mut status = _sender.send(vec![0.0; size].into());
                while status.is_err() {
                    tokio::task::yield_now().await;
                    status = _sender.send(status.err().unwrap().0);
                }
                tokio::time::sleep(cooldown).await;
            }
        });
        Self {
            sender,
            receiver: None,
            handle: Some(handle),
        }
    }
}

// A Node that just prints the data it receives and sends it forward
pub struct PrintNode<T: Clone + Send + Sync + Debug> {
    id: &'static str,
    sender: broadcast::Sender<T>,
    receiver: Option<broadcast::Receiver<T>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl<T: Clone + Send + Sync + Debug> Getters<T, T, ()> for PrintNode<T> {
    fn get_sender(&self) -> &broadcast::Sender<T> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<T>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.handle
    }
}

impl<T: Clone + Send + Sync + Debug + 'static> Node<T, T, ()> for PrintNode<T> {
    async fn follow<F: Clone + Send, I>(&mut self, node: &impl Node<F, T, I>) {
        self.unfollow().await;

        let sender = self.sender.clone();
        let mut receiver = node.subscribe();
        let id = self.id;

        let handle = tokio::spawn(async move {
            let mut counter: u64 = 0;
            loop {
                match receiver.recv().await {
                    Ok(data) => {
                        counter += 1;
                        if counter % 100_000 == 0 {
                            println!("{}: {:#?}", id, counter);
                        }
                        // println!("{}: {:#?}", id, data);
                        let _ = sender.send(data);
                    }
                    Err(e) => match e {
                        broadcast::error::RecvError::Closed => break,
                        broadcast::error::RecvError::Lagged(_) => (),
                    },
                }
            }
        });

        self.handle.replace(handle);
    }

    fn subscribe(&self) -> broadcast::Receiver<T> {
        self.get_sender().subscribe()
    }

    async fn unfollow(&mut self) {
        self.get_receiver().take();
        if let Some(handle) = self.get_handle().take() {
            handle.abort();
        }
    }
}

impl<T: Clone + Send + Sync + Debug> PrintNode<T> {
    pub fn new(id: &'static str) -> Self {
        let (sender, _) = broadcast::channel(CHANNEL_SIZE);
        Self {
            id,
            sender,
            receiver: None,
            handle: None,
        }
    }
}

pub async fn test_chain() {
    let zero = ArrayNode::new(Duration::from_secs_f64(2048.0 / 192_000.0), 4096 * 100);
    let window1 = Window::init(4096 * 10, 4096 * 10);
    let window2 = Window::init(4096, 4096);
    let window3 = Window::init(24523, 4096);
    let window4 = Window::init(4096, 4096);
    let window5 = Window::init(14351, 4096);
    let window6 = Window::init(134534, 4096);
    let window7 = Window::init(355452, 4096);
    let window8 = Window::init(34342, 4096);
    let window9 = Window::init(24342, 4096);
    let window10 = Window::init(4096, 4096);
    let mel_filter_bank = MelFilterBankNode::new(1000, 4096, 44100, 0.0, 22050.0);
    let printer: PrintNode<Arc<[f32]>> = PrintNode::new("FilterBank");
    let printer2: PrintNode<Arc<[f32]>> = PrintNode::new("Copys");
    let save_state = DashMap::<&str, NodeTypes>::new();
    save_state.insert("zero", zero.into());
    save_state.insert("window1", window1.into());
    save_state.insert("window2", window2.into());
    save_state.insert("window3", window3.into());
    save_state.insert("window4", window4.into());
    save_state.insert("window5", window5.into());
    save_state.insert("window6", window6.into());
    save_state.insert("window7", window7.into());
    save_state.insert("window8", window8.into());
    save_state.insert("window9", window9.into());
    save_state.insert("window10", window10.into());
    save_state.insert("mel_filter_bank", mel_filter_bank.into());
    save_state.insert("printer", printer.into());
    save_state.insert("printer2", printer2.into());

    save_state
        .get_mut("window1")
        .unwrap()
        .follow(&save_state.get("zero").unwrap())
        .await;
    save_state
        .get_mut("window2")
        .unwrap()
        .follow(&save_state.get("window1").unwrap())
        .await;
    save_state
        .get_mut("window3")
        .unwrap()
        .follow(&save_state.get("window1").unwrap())
        .await;
    save_state
        .get_mut("window4")
        .unwrap()
        .follow(&save_state.get("window3").unwrap())
        .await;
    save_state
        .get_mut("window5")
        .unwrap()
        .follow(&save_state.get("window4").unwrap())
        .await;
    save_state
        .get_mut("window6")
        .unwrap()
        .follow(&save_state.get("window5").unwrap())
        .await;
    save_state
        .get_mut("window7")
        .unwrap()
        .follow(&save_state.get("window6").unwrap())
        .await;
    save_state
        .get_mut("window8")
        .unwrap()
        .follow(&save_state.get("window7").unwrap())
        .await;
    save_state
        .get_mut("window9")
        .unwrap()
        .follow(&save_state.get("window8").unwrap())
        .await;
    save_state
        .get_mut("window10")
        .unwrap()
        .follow(&save_state.get("window9").unwrap())
        .await;
    save_state
        .get_mut("mel_filter_bank")
        .unwrap()
        .follow(&save_state.get("window4").unwrap())
        .await;
    save_state
        .get_mut("printer")
        .unwrap()
        .follow(&save_state.get("mel_filter_bank").unwrap())
        .await;
    save_state
        .get_mut("printer2")
        .unwrap()
        .follow(&save_state.get("window10").unwrap())
        .await;

    tokio::time::sleep(Duration::from_secs(10)).await;

    println!("Unfollowing");
    for mut value in save_state.iter_mut() {
        value.unfollow().await;
    }
}
