use std::sync::Arc;

#[allow(dead_code)]
pub struct WorkerPoolTokio<T> {
    rt: Arc<tokio::runtime::Runtime>,
    workers: Vec<tokio::task::JoinHandle<()>>,
    sender: kanal::AsyncSender<T>,
}

#[allow(dead_code)]
impl<T: Send + 'static> WorkerPoolTokio<T>{
    pub fn new<F>(num_workers: usize, f: F) -> Self
    where
        F: Fn(&tokio::runtime::Runtime, T) -> () + Send + Clone + 'static,
    {
        let (tx, rx) = kanal::unbounded_async();
        Self::with_channel(num_workers, tx, rx, f)
    }

    pub fn with_channel<F>(num_workers: usize, tx: kanal::AsyncSender<T>, rx: kanal::AsyncReceiver<T>, f: F) -> Self
    where
        F: Fn(&tokio::runtime::Runtime, T) -> () + Send + Clone + 'static,
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_workers)
            .enable_all()
            .build()
            .unwrap();
        let rt = Arc::new(rt);
        let workers = (0..num_workers)
            .map(|_| {
                let f_inner = f.clone();
                let rx = rx.clone();
                let rt_inner = rt.clone();
                rt.spawn(async move {
                    while let Ok(data) = rx.recv().await {
                        f_inner(&rt_inner, data);
                    }
                })
            })
            .collect();
        Self {
            rt,
            workers,
            sender: tx,
        }
    }

    pub fn get_sender(&self) -> kanal::AsyncSender<T> {
        self.sender.clone()
    }
}

