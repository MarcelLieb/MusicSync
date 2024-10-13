use std::sync::Arc;

use log::warn;
use tokio::sync::broadcast;
mod audio;
mod general;
pub mod test;

const CHANNEL_SIZE: usize = 32;

pub trait NodeTrait<I: Clone + Send, O: Clone + Send, S>: internal::Getters<I, O, S> + Send {
    fn subscribe(&self) -> broadcast::Receiver<O> {
        self.get_sender().subscribe()
    }
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, I, F>);
    async fn unfollow(&mut self) {
        self.get_receiver().take();
        if let Some(handle) = self.get_handle().take() {
            handle.abort();
        }
    }
}

mod internal {
    use tokio::sync::broadcast;

    pub trait Getters<I: Clone + Send, O: Clone + Send, T> {
        fn get_sender(&self) -> &broadcast::Sender<O>;
        fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<I>>;
        fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<T>>;
    }
}

pub trait FallibleNode<I: Clone + Send, O: Clone + Send> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, I, F>);
}


#[non_exhaustive]
enum Node {
    Aggregate(general::array::Aggregate<f32>),
    Window(general::array::Window<f32>),
    RetimerFloat(general::array::Retimer<f32>),
    RetimerArray(general::array::Retimer<Arc<[f32]>>),
    MelFilterBank(audio::filterbank::MelFilterBankNode<f32>),
    Zero(test::ZeroNode),
    Array(test::ArrayNode),
    PrinterFloat(test::PrintNode<f32>),
    PrinterArray(test::PrintNode<Arc<[f32]>>),
    FFT(audio::fft::FFT),
}

impl FallibleNode<f32, f32> for Node {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, f32, F>) {
        match self {
            Node::Aggregate(_node) => _node.follow(node).await,
            Node::RetimerFloat(_node) => _node.follow(node).await,
            Node::PrinterFloat(_node) => _node.follow(node).await,
            _ => {}
        }
    }
}

impl FallibleNode<Arc<[f32]>, Arc<[f32]>> for Node {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, Arc<[f32]>, F>) {
        match self {
            Node::RetimerArray(_node) => _node.follow(node).await,
            Node::PrinterArray(_node) => _node.follow(node).await,
            Node::MelFilterBank(_node) => _node.follow(node).await,
            Node::Window(_node) => _node.follow(node).await,
            Node::FFT(_node) => _node.follow(node).await,
            _ => {}
        }
    }
}

impl Node {
    pub async fn follow(&mut self, node: &Node) {
        match node {
            Node::Aggregate(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::Window(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::MelFilterBank(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::Array(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::RetimerArray(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::PrinterFloat(node) => FallibleNode::<f32, f32>::follow(self, node).await,
            Node::PrinterArray(node) => {
                FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await
            }
            Node::RetimerFloat(node) => FallibleNode::<f32, f32>::follow(self, node).await,
            Node::Zero(node) => FallibleNode::<f32, f32>::follow(self, node).await,
            Node::FFT(node) => FallibleNode::<Arc<[f32]>, Arc<[f32]>>::follow(self, node).await,
        }
    }

    pub async fn unfollow(&mut self) {
        match self {
            Node::Aggregate(node) => node.unfollow().await,
            Node::Window(node) => node.unfollow().await,
            Node::MelFilterBank(node) => node.unfollow().await,
            Node::Array(node) => node.unfollow().await,
            Node::RetimerArray(node) => node.unfollow().await,
            Node::PrinterFloat(node) => node.unfollow().await,
            Node::PrinterArray(node) => node.unfollow().await,
            Node::RetimerFloat(node) => node.unfollow().await,
            Node::Zero(node) => node.unfollow().await,
            Node::FFT(node) => node.unfollow().await,
        }
    }
}

impl From<general::array::Aggregate<f32>> for Node {
    fn from(node: general::array::Aggregate<f32>) -> Self {
        Node::Aggregate(node)
    }
}

impl From<general::array::Window<f32>> for Node {
    fn from(node: general::array::Window<f32>) -> Self {
        Node::Window(node)
    }
}

impl From<general::array::Retimer<f32>> for Node {
    fn from(node: general::array::Retimer<f32>) -> Self {
        Node::RetimerFloat(node)
    }
}

impl From<general::array::Retimer<Arc<[f32]>>> for Node {
    fn from(node: general::array::Retimer<Arc<[f32]>>) -> Self {
        Node::RetimerArray(node)
    }
}

impl From<audio::filterbank::MelFilterBankNode<f32>> for Node {
    fn from(node: audio::filterbank::MelFilterBankNode<f32>) -> Self {
        Node::MelFilterBank(node)
    }
}

impl From<test::ZeroNode> for Node {
    fn from(node: test::ZeroNode) -> Self {
        Node::Zero(node)
    }
}

impl From<test::ArrayNode> for Node {
    fn from(node: test::ArrayNode) -> Self {
        Node::Array(node)
    }
}

impl From<test::PrintNode<f32>> for Node {
    fn from(node: test::PrintNode<f32>) -> Self {
        Node::PrinterFloat(node)
    }
}

impl From<test::PrintNode<Arc<[f32]>>> for Node {
    fn from(node: test::PrintNode<Arc<[f32]>>) -> Self {
        Node::PrinterArray(node)
    }
}

impl From<audio::fft::FFT> for Node {
    fn from(node: audio::fft::FFT) -> Self {
        Node::FFT(node)
    }
}

struct NodeImpl<I: Clone, O: Clone> {
    sender: broadcast::Sender<O>,
    receiver: Option<broadcast::Receiver<I>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl<I: Clone + Send + 'static, O: Clone + Send + 'static> internal::Getters<I, O, ()>
    for NodeImpl<I, O>
{
    fn get_sender(&self) -> &broadcast::Sender<O> {
        &self.sender
    }

    fn get_receiver(&mut self) -> &mut Option<broadcast::Receiver<I>> {
        &mut self.receiver
    }

    fn get_handle(&mut self) -> &mut Option<tokio::task::JoinHandle<()>> {
        &mut self.handle
    }
}

impl<I: Clone + Send + 'static> NodeTrait<I, I, ()> for NodeImpl<I, I> {
    async fn follow<T: Clone + Send, F>(&mut self, node: &impl NodeTrait<T, I, F>) {
        let mut receiver = node.subscribe();

        if let Some(handle) = self.handle.take() {
            handle.abort();
        }

        let sender = self.sender.clone();
        let handle = tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(data) => {
                        let _ = sender.send(data);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Lagged by {}", n);
                    }
                }
            }
        });
        self.handle.replace(handle);
    }
}
