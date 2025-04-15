use std::{collections::VecDeque, sync::{Arc, Mutex}};

use log::warn;

use crate::nodes::{DataHandler, PortId};

pub struct Aggregate<I> {
    buffer: Mutex<VecDeque<I>>,
    size: usize,
    hop_size: usize,
}

impl<I> Aggregate<I> {
    pub fn init(size: usize, hop_size: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
            size,
            hop_size,
        }
    }
}

impl DataHandler for Aggregate<f32> {
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::Float(data) => {
                let mut buffer = self.buffer.lock().unwrap();
                buffer.push_back(data);
                if buffer.len() >= self.size {
                    let data: Arc<[f32]> = Arc::from(buffer.make_contiguous()[..self.size].to_vec());
                    buffer.drain(0..self.hop_size);
                    vec![(0, crate::nodes::Data::FloatArray(data.into()))]
                } else {
                    vec![]
                }
            }
            _ => {
                warn!("Invalid data type");
                vec![]
            }
        }
    }

    fn num_input_ports(&self) -> PortId {
        1
    }

    fn num_output_ports(&self) -> PortId {
        1
    }

    fn get_input_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }
}

pub struct Window<I> {
    buffer: Mutex<VecDeque<I>>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send> Window<I> {
    pub fn init(size: usize, hop_size: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
            size,
            hop_size,
        }
    }
}

impl DataHandler for Window<f32> {
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                let mut out = Vec::new();
                {
                    let mut buffer = self.buffer.lock().unwrap();
                    buffer.extend(data.iter());
                    while buffer.len() >= self.size {
                        let data: Arc<[f32]> = Arc::from(buffer.make_contiguous()[..self.size].to_vec());
                        out.push((0, crate::nodes::Data::FloatArray(data.into())));
                        buffer.drain(0..self.hop_size);
                    }
                }
                out
            }
            _ => {
                warn!("Invalid data type");
                vec![]
            }
        }
    }

    fn num_input_ports(&self) -> PortId {
        1
    }

    fn num_output_ports(&self) -> PortId {
        1
    }

    fn get_input_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }
}
