use std::sync::{Arc, Mutex};

use log::warn;

use crate::nodes::DataHandler;



struct SpecFlux {
    prev: Mutex<Arc<[f32]>>,
}

impl SpecFlux {
    pub fn init() -> Self {
        Self {
            prev: Mutex::new(Arc::new([])),
        }
    }
}

impl DataHandler for SpecFlux {
    fn handle(&self, port: usize, data: crate::nodes::Data) -> Vec<(usize, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                let out = {
                    let mut prev = self.prev.lock().unwrap();
                    let out = data.iter().zip(prev.iter()).map(|(a, b)| (a - b).max(0.0)).sum();
                    *prev = data.clone();
                    out
                };
                vec![(0, crate::nodes::Data::Float(out))]
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
            0 => Some("Frequencies".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Activation".into()),
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
            0 => Some(crate::nodes::DataType::Float),
            _ => None,
        }
    }
}

pub struct HFC {
    bin_resolution: f32,
}

impl HFC {
    pub fn new(sample_rate: usize, fft_size: usize) -> Self {
        let bin_resolution = sample_rate as f32 / fft_size as f32;
        Self {
            bin_resolution,
        }
    }
}

impl DataHandler for HFC {
    fn handle(&self, port: usize, data: crate::nodes::Data) -> Vec<(usize, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                let out = data.iter().enumerate().map(|(k, freq)| k as f32 * self.bin_resolution * freq).sum();
                vec![(0, crate::nodes::Data::Float(out))]
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
            0 => Some("FFT".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        match port {
            0 => Some("Activation".into()),
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
            0 => Some(crate::nodes::DataType::Float),
            _ => None,
        }
    }
    
}