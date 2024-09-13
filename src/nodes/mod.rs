use log::warn;
use tokio::sync::broadcast;
mod general;

pub trait Node<I: Clone + Send, O: Clone + Send, S>: internal::Getters<I, O, S> {
    fn subscribe(&self) -> broadcast::Receiver<O> {
        self.get_sender().subscribe()
    }
    fn follow<T: Clone + Send, F>(&mut self, node: impl Node<T, I, F>);
    fn unfollow(&mut self) {
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

impl<I: Clone + Send + 'static> Node<I, I, ()> for NodeImpl<I, I> {
    fn follow<T: Clone + Send, F>(&mut self, node: impl Node<T, I, F>) {
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
