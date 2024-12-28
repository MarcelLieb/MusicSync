use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex, RwLock},
};

use kanal::Sender;
use uuid::Uuid;

use crate::utils::worker_pool::WorkerPoolStd;
mod audio;
mod general;
pub mod test;

pub enum DataType {
    Float,
    FloatArray,
    Color,
    ColorArray,
    Int,
    IntArray,
}

#[derive(Debug, Clone)]
pub enum Data {
    Float(f32),
    FloatArray(Arc<[f32]>),
    Color((u8, u8, u8)),
    ColorArray(Arc<[(u8, u8, u8)]>),
    Int(i32),
    IntArray(Arc<[i32]>),
}

type Address = (Arc<str>, usize);

pub trait DataHandler {
    fn handle(&mut self, port: usize, data: Data) -> Vec<(usize, Data)>;

    fn num_input_ports(&self) -> usize;

    fn num_output_ports(&self) -> usize;

    fn get_input_name(&self, port: usize) -> Option<Arc<str>>;

    fn get_output_name(&self, port: usize) -> Option<Arc<str>>;

    fn get_input_type(&self, port: usize) -> Option<DataType>;

    fn get_output_type(&self, port: usize) -> Option<DataType>;
}

pub struct DataGraph {
    nodes: HashMap<Arc<str>, Arc<Mutex<dyn DataHandler + Send>>>,
    follow_graph: HashMap<Arc<str>, Arc<[RwLock<Vec<(Arc<str>, usize)>>]>>,
}

impl DataGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            follow_graph: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, handler: impl DataHandler + Send + 'static) -> Arc<str> {
        let port_count = handler.num_output_ports();
        let id: Arc<str> = self.add_reference(port_count);
        self.nodes.insert(id.clone(), Arc::new(Mutex::new(handler)));
        id
    }

    fn add_reference(&mut self, num_ports: usize) -> Arc<str> {
        let id: Arc<str> = Uuid::new_v4().to_string().into();
        self.follow_graph.insert(id.clone(), (0..num_ports).map(|_| RwLock::new(Vec::new())).collect::<Arc<[_]>>());
        id
    }

    pub fn follow(&self, source: &str, target: &str, port_1: usize, port_2: usize) {
        let follow_graph = &self.follow_graph.get(source).unwrap()[port_1];
        let mut follow_graph = follow_graph.write().unwrap();
        follow_graph.push((target.into(), port_2));
    }

    pub fn handle_data(&self, address: Address, data: Data) -> Vec<(Address, Data)> {
        let node_id = Arc::from(address.0);
        let node = self.nodes.get(&node_id).unwrap();
        let data = {
            let mut node = node.lock().unwrap();
            node.handle(address.1, data)
        };

        let followers_per_port = self.follow_graph.get(&node_id).unwrap();
        let results = data.into_iter().flat_map(|(port, data)| {
            let followers = followers_per_port[port].read().unwrap();
            followers.iter().map(|(follower, port)| ((follower.clone(), *port), data.clone())).collect::<Vec<_>>()
        }).collect();
        return results;
    }

    pub fn get_followers(&self, node: &str, port: usize) -> Option<Vec<(Arc<str>, usize)>> {
        let followers_per_port = self.follow_graph.get(node)?;
        let followers = followers_per_port[port].read().unwrap();
        Some(followers.clone())
    }
}

pub struct DataGraphManager {
    graph: Arc<RwLock<DataGraph>>,
    work_dispatcher: std::thread::JoinHandle<()>,
    input_dispatcher: std::thread::JoinHandle<()>,
    work_queue: Sender<(Address, Data)>,
    input_queue: Sender<(Address, Data)>,
}

impl DataGraphManager {
    pub fn new() -> Self {
        let graph = Arc::new(RwLock::new(DataGraph::new()));
        let graph_inner = graph.clone();

        let (tx, rx) = kanal::unbounded::<(Address, Data)>();
        let tx_inner = tx.clone();

        let work_dispatcher = std::thread::spawn(move || {
            let _pool: WorkerPoolStd<((Arc<str>, usize), Data)> = WorkerPoolStd::with_channel(16, tx_inner.clone(), rx, move |(address, data): (Address, Data)| {
                let graph_inner = graph_inner.clone();
                let tx_inner = tx_inner.clone();
                let outputs = {
                    let graph = graph_inner.read().unwrap();
                    graph.handle_data(address, data)
                };
                for (address, data) in outputs {
                    tx_inner.send((address, data)).unwrap();
                }
            });
        });

        let (in_tx, in_rx) = kanal::unbounded::<(Address, Data)>();
        let graph_inner = graph.clone();
        let tx_inner = tx.clone();
        let input_dispatcher = std::thread::spawn(move || {
            while let Ok((address, data)) = in_rx.recv() {
                let followers = {
                    let graph = graph_inner.read().unwrap();
                    graph.get_followers(&address.0, address.1)
                };
                if let Some(followers) = followers {
                    for (follower, port) in followers {
                        tx_inner.send(((follower, port), data.clone())).unwrap();
                    }
                }
            }
        });

        let tx = tx.clone();

        Self {
            graph,
            work_dispatcher,
            input_dispatcher,
            work_queue: tx,
            input_queue: in_tx,
        }
    }

    pub fn handle_data(&self, address: Address, data: Data) {
        self.work_queue.send((address, data)).unwrap();
    }

    pub fn add_node(&mut self, handler: impl DataHandler + Send + 'static) -> Arc<str> {
        self.graph.write().unwrap().add_node(handler)
    }

    pub fn add_reference(&self, ports: usize) -> Arc<str> {
        self.graph.write().unwrap().add_reference(ports)
    }

    pub fn get_input_queue(&self) -> Sender<(Address, Data)> {
        self.input_queue.clone()
    }

    pub fn follow(&self, follower: &str, followee: &str, port_1: usize, port_2: usize) {
        self.graph.read().unwrap().follow(follower, followee, port_1, port_2);
    }
}

pub struct PrintNode {
    count: usize,
    every: usize,
}

impl PrintNode {
    pub fn new(every: usize) -> Self {
        Self { count: 0, every }
    }
}

impl DataHandler for PrintNode {
    fn handle(&mut self, port: usize, data: Data) -> Vec<(usize, Data)> {
        if port != 0 {
            return vec![];
        }
        self.count += 1;
        if self.count % self.every == 0 {
            println!("{:?}", data);
            self.count = 0;
        }
        vec![(port, data)]
    }

    fn num_input_ports(&self) -> usize {
        1
    }

    fn num_output_ports(&self) -> usize {
        1
    }

    fn get_input_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("input".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("output".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: usize) -> Option<DataType> {
        match port {
            0 => Some(DataType::Float),
            _ => None,
        }
    }

    fn get_output_type(&self, port: usize) -> Option<DataType> {
        match port {
            0 => Some(DataType::Float),
            _ => None,
        }
    }
}
