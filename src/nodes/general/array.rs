use std::{collections::VecDeque, sync::Arc};

use log::warn;

use crate::nodes::DataHandler;

pub struct Aggregate<I> {
    buffer: VecDeque<I>,
    size: usize,
    hop_size: usize,
}

impl<I> Aggregate<I> {
    pub fn init(size: usize, hop_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            size,
            hop_size,
        }
    }
}

impl DataHandler for Aggregate<f32> {
    fn handle(&mut self, port: usize, data: crate::nodes::Data) -> Vec<(usize, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::Float(data) => {
                self.buffer.push_back(data);
                if self.buffer.len() >= self.size {
                    let data: Arc<[f32]> = Arc::from(self.buffer.make_contiguous()[..self.size].to_vec());
                    self.buffer.drain(0..self.hop_size);
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

    fn num_input_ports(&self) -> usize {
        1
    }

    fn num_output_ports(&self) -> usize {
        1
    }

    fn get_input_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Input".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Output".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: usize) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: usize) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }
}

pub struct Window<I> {
    buffer: VecDeque<I>,
    size: usize,
    hop_size: usize,
}

impl<I: Clone + Send> Window<I> {
    pub fn init(size: usize, hop_size: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            size,
            hop_size,
        }
    }
}

impl DataHandler for Window<f32> {
    fn handle(&mut self, port: usize, data: crate::nodes::Data) -> Vec<(usize, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                self.buffer.extend(data.iter());
                if self.buffer.len() >= self.size {
                    let data: Arc<[f32]> = Arc::from(self.buffer.make_contiguous()[..self.size].to_vec());
                    self.buffer.drain(0..self.hop_size);
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

    fn num_input_ports(&self) -> usize {
        1
    }

    fn num_output_ports(&self) -> usize {
        1
    }

    fn get_input_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Input".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Output".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: usize) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: usize) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }
}
