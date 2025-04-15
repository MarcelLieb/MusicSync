use std::sync::{Arc, Mutex};

use log::warn;

use crate::nodes::{DataHandler, PortId};



pub struct SpecFlux {
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
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                let log_magnitude = data.iter().map(|x| (x * 0.1).ln_1p()).collect::<Arc<[f32]>>();
                let out = {
                    let mut prev = self.prev.lock().unwrap();
                    let out = prev.iter().zip(log_magnitude.iter()).map(|(a, b)| (b - a).max(0.0)).sum();
                    *prev = log_magnitude.clone();
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

    fn num_input_ports(&self) -> PortId {
        1
    }

    fn num_output_ports(&self) -> PortId {
        1
    }

    fn get_input_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("Frequencies".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("Activation".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
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
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
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

    fn num_input_ports(&self) -> PortId {
        1
    }

    fn num_output_ports(&self) -> PortId {
        1
    }

    fn get_input_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("FFT".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("Activation".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 => Some(crate::nodes::DataType::Float),
            _ => None,
        }
    }
    
}