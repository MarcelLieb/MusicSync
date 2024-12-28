use std::{collections::HashMap, fmt::Debug, sync::{Arc, Mutex, RwLock}};

use dashmap::DashMap;
use rayon::ThreadPoolBuilder;
use uuid::Uuid;
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

    fn get_input_name(&self, port: usize) -> Option<&str>;

    fn get_output_name(&self, port: usize) -> Option<&str>;

    fn get_input_type(&self, port: usize) -> Option<DataType>;

    fn get_output_type(&self, port: usize) -> Option<DataType>;
}

pub struct DataGraph {
    nodes: DashMap<Arc<str>, Arc<Mutex<dyn DataHandler + Send>>>,
    follow_graph: DashMap<Arc<str>, RwLock<HashMap<usize, Vec<(Arc<str>, usize)>>>>,
}

impl DataGraph {
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new(),
            follow_graph: DashMap::new(),
        }
    }

    pub fn add_node(&self, handler: impl DataHandler + Send + 'static) -> Arc<str> {
        let id: Arc<str> = Uuid::new_v4().to_string().into();
        self.nodes.insert(id.clone(), Arc::new(Mutex::new(handler)));
        self.follow_graph.insert(id.clone(), RwLock::new(HashMap::new()));
        id
    }

    pub fn follow(&self, follower: &str, followee: &str, port_1: usize, port_2: usize) {
        println!("{:?}", self.follow_graph);
        println!("{} -> {} {} {}\n", follower, followee, port_1, port_2);
        let follow_graph = self.follow_graph.get(follower).unwrap();
        let mut follow_graph = follow_graph.write().unwrap();
        let followers = follow_graph.entry(port_1).or_insert_with(Vec::new);
        followers.push((followee.into(), port_2));
    }

    pub fn handle_data(&self, address: Address, data: Data) -> Vec<(Address, Data)> {
        let node_id = Arc::from(address.0);
        let node = self.nodes.get(&node_id).unwrap();
        let mut node = node.lock().unwrap();
        let data = node.handle(address.1, data);
        let followers_per_port = self.follow_graph.get(&node_id).unwrap();
        let followers_per_port = followers_per_port.read().unwrap();
        let results = data
            .into_iter()
            .flat_map(|(port, data)| {
                let followers = followers_per_port.get(&port);
                followers.map(|followers| {
                    followers
                        .iter()
                        .map(move |(follower, port)| ((follower.clone(), *port), data.clone()))
                })
            })
            .flatten()
            .collect();
        return results;
    }
}

pub struct DataGraphManager {
    graph: Arc<DataGraph>,
    handle: std::thread::JoinHandle<()>,
    work_queue: std::sync::mpsc::Sender<(Address, Data)>,
}

impl DataGraphManager {
    pub fn new() -> Self {
        let graph = Arc::new(DataGraph::new());
        let graph_inner = graph.clone();

        let (tx, rx) = std::sync::mpsc::channel::<(Address, Data)>();
        let tx_inner = tx.clone();

        let handle = std::thread::spawn(move || {
            let pool = ThreadPoolBuilder::new().num_threads(16).build().unwrap();
            pool.scope(move |s| {
                while let Ok((address, data)) = rx.recv() {
                    let tx_inner = tx_inner.clone();
                    let graph_inner = graph_inner.clone();
                    s.spawn(move |_| {
                        for (address, data) in graph_inner.handle_data(address, data) {
                            tx_inner.send((address, data)).unwrap();
                        }
                    });
                }
            });
        });
        Self {
            graph,
            handle,
            work_queue: tx,
        }
    }

    pub fn handle_data(&self, address: Address, data: Data) {
        self.work_queue.send((address, data)).unwrap();
    }

    pub fn add_node(&self, handler: impl DataHandler + Send + 'static) -> Arc<str> {
        self.graph.add_node(handler)
    }

    pub fn follow(&self, follower: &str, followee: &str, port_1: usize, port_2: usize) {
        self.graph.follow(follower, followee, port_1, port_2);
    }
}

pub struct PrintNode { 
    count: usize,
    every: usize,
}

impl PrintNode {
    pub fn new(every: usize) -> Self {
        Self {
            count: 0,
            every,
        }
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

    fn get_input_name(&self, port: usize) -> Option<&str> {
        match port {
            0 => Some("input"),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<&str> {
        match port {
            0 => Some("output"),
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
