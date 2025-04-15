use std::sync::Arc;

use parking_lot::Mutex;

use crate::{nodes::{Data, DataHandler, DataType, PortId}, utils::audioprocessing::threshold};



pub struct PeakPickingNode {
    settings: threshold::BasicSettings,
    picker: Mutex<threshold::Basic>,
}

impl PeakPickingNode {
    pub fn new(settings: threshold::BasicSettings) -> Self {
        let picker = threshold::Basic::with_settings(settings);
        Self {
            settings,
            picker: Mutex::new(picker),
        }
    }
}

impl DataHandler for PeakPickingNode {
    fn handle(&self, port: PortId, data: Data) -> Vec<(PortId, Data)> {
        if port != 0 {
            log::warn!("Invalid port");
            return vec![];
        }
        match data {
            Data::Float(data) => {
                let peak = {
                    let mut picker = self.picker.lock();
                    picker.is_above(data)
                };
                if peak {
                    vec![(0, Data::Float(data))]
                } else {
                    vec![]
                }
            }
            _ => {
                log::warn!("Invalid data type");
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
            0 => Some("Activation".into()),
            _ => None,
        }
    }
    
    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("Peaks".into()),
            _ => None,
        }
    }
    
    fn get_input_type(&self, port: PortId) -> Option<DataType> {
        match port {
            0 => Some(DataType::Float),
            _ => None,
        }
    }
    
    fn get_output_type(&self, port: PortId) -> Option<DataType> {
        match port {
            0 => Some(DataType::Float),
            _ => None,
        }
    }
    
}