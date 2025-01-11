use std::{
    collections::HashMap, fmt::Debug, sync::{atomic::{AtomicU16, AtomicUsize, Ordering}, Arc, RwLock}
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

#[derive(Debug)]
pub enum Data {
    Float(f32),
    FloatArray(Arc<[f32]>),
    Color((u8, u8, u8)),
    ColorArray(Arc<[(u8, u8, u8)]>),
    Int(i32),
    IntArray(Arc<[i32]>),
}

impl Clone for Data {
    fn clone(&self) -> Self {
        match self {
            Self::Float(arg0) => Self::Float(arg0.clone()),
            Self::Color(arg0) => Self::Color(arg0.clone()),
            Self::Int(arg0) => Self::Int(arg0.clone()),
            // Make sure the Arc clone is used
            Self::FloatArray(arg0) => Self::FloatArray(Arc::clone(arg0)),
            Self::ColorArray(arg0) => Self::ColorArray(Arc::clone(arg0)),
            Self::IntArray(arg0) => Self::IntArray(Arc::clone(arg0)),
        }
    }
}

type Address = (u128, usize);

pub trait DataHandler {
    fn handle(&self, port: usize, data: Data) -> Vec<(usize, Data)>;

    fn num_input_ports(&self) -> usize;

    fn num_output_ports(&self) -> usize;

    fn get_input_name(&self, port: usize) -> Option<Arc<str>> {
        let num_ports = self.num_input_ports();
        if port >= num_ports {
            return None;
        }
        if num_ports > 1 {
            return Some(format!("Input {}", port).into())
        }
        Some("Input".into())
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        let num_ports = self.num_output_ports();
        if port >= num_ports {
            return None;
        }
        if num_ports > 1 {
            return Some(format!("Output {}", port).into())
        }
        Some("Output".into())
    }

    fn get_input_type(&self, port: usize) -> Option<DataType>;

    fn get_output_type(&self, port: usize) -> Option<DataType>;
}

pub struct DataGraph {
    nodes: HashMap<u128, Arc<dyn DataHandler + Send + Sync>>,
    follow_graph: HashMap<u128, Arc<[RwLock<Vec<(u128, usize)>>]>>,
}

impl DataGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            follow_graph: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, handler: impl DataHandler + Send + Sync + 'static) -> u128 {
        let port_count = handler.num_output_ports();
        let id:u128 = self.add_reference(port_count);
        self.nodes.insert(id.clone(), Arc::new(handler));
        id
    }

    fn add_reference(&mut self, num_ports: usize) -> u128 {
        let id: u128 = Uuid::new_v4().to_u128_le();
        self.follow_graph.insert(id, (0..num_ports).map(|_| RwLock::new(Vec::new())).collect::<Arc<[_]>>());
        id
    }

    pub fn follow(&self, source: u128, target: u128, port_1: usize, port_2: usize) {
        let follow_graph = &self.follow_graph.get(&source).unwrap()[port_1];
        let mut follow_graph = follow_graph.write().unwrap();
        follow_graph.push((target.into(), port_2));
    }

    pub fn handle_data(&self, address: Address, data: Data) -> Vec<(Address, Data)> {
        let node_id = Arc::from(address.0);
        let node = self.nodes.get(&node_id).unwrap();
        let data = node.handle(address.1, data);

        let followers_per_port = self.follow_graph.get(&node_id).unwrap();
        let results = data.into_iter().flat_map(|(port, data)| {
            let followers = followers_per_port[port].read().unwrap();
            followers.iter().map(|(follower, port)| ((follower.clone(), *port), data.clone())).collect::<Vec<_>>()
        }).collect();
        return results;
    }

    pub fn get_followers(&self, node: u128, port: usize) -> Option<Vec<(u128, usize)>> {
        let followers_per_port = self.follow_graph.get(&node)?;
        let followers = followers_per_port[port].read().unwrap();
        Some(followers.clone())
    }
}

pub struct DataGraphManager {
    work_queue: Sender<(usize, (Address, Data))>,
    graph: Arc<RwLock<DataGraph>>,
    time_index: Arc<AtomicU16>,
    input_queue: Sender<(Address, Data)>,
    worker_pool: WorkerPoolStd<(Address, Data)>,
    input_dispatcher: std::thread::JoinHandle<()>,
}

impl DataGraphManager {
    pub fn new() -> Self {
        let graph = Arc::new(RwLock::new(DataGraph::new()));
        let graph_inner = graph.clone();
        let time_index = Arc::new(AtomicU16::new(0));

        let (tx, rx) = kanal::unbounded::<(usize, (Address, Data))>();
        let tx_inner = tx.clone();

        let pool: WorkerPoolStd<((u128, usize), Data)> = WorkerPoolStd::with_channel(12, tx_inner.clone(), rx, move |prio, (address, data)| {
            let outputs = {
                let graph = graph_inner.read().unwrap();
                graph.handle_data(address, data)
            };
            for (address, data) in outputs {
                let _ = tx_inner.send((prio + 1, (address, data)));
            }
        });

        let (in_tx, in_rx) = kanal::unbounded::<(Address, Data)>();
        let graph_inner = graph.clone();
        let tx_inner = tx.clone();
        let time_inner = time_index.clone();
        let input_dispatcher = std::thread::spawn(move || {
            while let Ok((address, data)) = in_rx.recv() {
                let time = time_inner.fetch_sub(1, Ordering::Acquire) as u32;
                let followers = {
                    let graph = graph_inner.read().unwrap();
                    graph.get_followers(address.0, address.1)
                };
                if let Some(followers) = followers {
                    for (follower, port) in followers {
                        tx_inner.send(((time << 16) as usize, ((follower, port), data.clone()))).unwrap();
                    }
                }
            }
        });

        let tx = tx.clone();

        Self {
            graph,
            worker_pool: pool,
            input_dispatcher,
            work_queue: tx,
            input_queue: in_tx,
            time_index: Arc::new(AtomicU16::new(0))
        }
    }

    pub fn handle_data(&self, address: Address, data: Data) {
        // Max heap prioritize older data
        let time = self.time_index.fetch_sub(1, Ordering::Acquire) as u32;
        let _ = self.work_queue.send(((time << 16) as usize, (address, data)));
    }

    pub fn add_node(&mut self, handler: impl DataHandler + Send + Sync + 'static) -> u128 {
        self.graph.write().unwrap().add_node(handler)
    }

    pub fn add_reference(&self, ports: usize) -> u128 {
        self.graph.write().unwrap().add_reference(ports)
    }

    pub fn get_input_queue(&self) -> Sender<(Address, Data)> {
        self.input_queue.clone()
    }

    pub fn follow(&self, follower: u128, followee: u128, port_1: usize, port_2: usize) {
        self.graph.read().unwrap().follow(follower, followee, port_1, port_2);
    }
}

pub struct PrintNode {
    count: AtomicUsize,
    every: usize,
}

impl PrintNode {
    pub fn new(every: usize) -> Self {
        Self { count: 0.into(), every }
    }
}

impl DataHandler for PrintNode {
    fn handle(&self, port: usize, data: Data) -> Vec<(usize, Data)> {
        if port != 0 {
            return vec![];
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        if self.count.load(Ordering::Relaxed) % self.every == 0 {
            println!("{:?}", data);
            self.count.store(0, Ordering::Relaxed);
        }
        vec![(port, data)]
    }

    fn num_input_ports(&self) -> usize {
        1
    }

    fn num_output_ports(&self) -> usize {
        1
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
