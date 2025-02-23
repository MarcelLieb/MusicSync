use std::{collections::BinaryHeap, sync::Arc};
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


enum PrioT<T> {
    Tuple((usize, T)),
}

impl<T> PrioT<T> {
    fn unwrap(self) -> (usize, T) {
        match self {
            PrioT::Tuple((prio, data)) => (prio, data),
        }
    }
}

impl<T> From<(usize, T)> for PrioT<T> {
    fn from(tuple: (usize, T)) -> Self {
        PrioT::Tuple(tuple)
    }
}

impl<T> Ord for PrioT<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (PrioT::Tuple((prio1, _)), PrioT::Tuple((prio2, _))) => prio1.cmp(prio2),
        }
    }
}

impl<T> PartialOrd for PrioT<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> PartialEq for PrioT<T> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (PrioT::Tuple((prio1, _)), PrioT::Tuple((prio2, _))) => prio1 == prio2,
        }
    }
}

impl<T> Eq for PrioT<T> {}

impl<T: Clone> Clone for PrioT<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Tuple(arg0) => Self::Tuple(arg0.clone()),
        }
    }
}

#[allow(dead_code)]
pub struct WorkerPoolStd<T> {
    workers: Vec<std::thread::JoinHandle<()>>,
    pub sender: kanal::Sender<(usize, T)>,
    inner_sender: kanal::Sender<(usize, T)>,
    inner_receiver: kanal::Receiver<(usize, T)>,
    dispatcher: std::thread::JoinHandle<()>,
}

#[allow(dead_code)]
impl<T: Clone + Send + 'static> WorkerPoolStd<T> {
    pub fn new<F>(num_workers: usize, f: F) -> Self
    where
        F: Fn(usize, T) -> () + Send + Clone + 'static,
    {
        let (tx, rx) = kanal::unbounded();
        Self::with_channel(num_workers, tx, rx, f)
    }

    pub fn with_channel<F>(num_workers: usize, tx: kanal::Sender<(usize, T)>, rx: kanal::Receiver<(usize, T)>, f: F) -> Self
    where
        F: Fn(usize, T) -> () + Send + Clone + 'static,
    {
        let (inner_sender, inner_receiver) = kanal::bounded(0);
        let inner_tx = inner_sender.clone();
        let dispatcher = std::thread::Builder::new().name("Dispatcher".into()).spawn(move || {
            let mut work_queue: BinaryHeap<PrioT<T>> = BinaryHeap::new();
            let batch_size = 2;
            loop {
                if let Some(data) = work_queue.pop() {
                    // If there is data queued try to distribute it to the threads
                    let mut option = Option::Some(data.unwrap());
                    if let Ok(success) = inner_tx.try_send_option(&mut option) {
                        if !success {
                            // If all threads are busy readd it to the queue
                            work_queue.push(option.unwrap().into());
                        }
                        // Check for new data without blocking
                        for _ in 0..batch_size {
                            if let Ok(Some(data)) = rx.try_recv() {
                                work_queue.push(data.into());
                            }
                        }
                    } else {
                        break;
                    }
                } else {
                    // If there is no data queued wait for new data
                    if let Ok(data) = rx.recv() {
                        work_queue.push(data.into());
                    } else {
                        break;
                    }
                }
            }
        }).unwrap();
        let workers = (0..num_workers)
            .map(|_| {
                let f_inner = f.clone();
                let rx = inner_receiver.clone();
                std::thread::Builder::new().name("Worker".into()).spawn(move || {
                    while let Ok((prio, data)) = rx.recv() {
                        f_inner(prio, data);
                    }
                }).unwrap()
            })
            .collect();
        Self { workers, sender: tx, inner_sender, inner_receiver, dispatcher }
    }
}

impl<T> Drop for WorkerPoolStd<T> {
    fn drop(&mut self) {
        self.inner_sender.close();
        self.inner_receiver.close();
        self.sender.close();
        while let Some(worker) = self.workers.pop() {
            worker.join().unwrap();
        }
    }
}

