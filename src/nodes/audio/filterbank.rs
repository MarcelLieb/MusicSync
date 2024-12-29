use std::sync::Arc;

use log::warn;

use crate::{
    nodes::DataHandler, utils::audioprocessing::MelFilterBank
};

pub struct MelFilterBankNode {
    filter_bank: MelFilterBank,
}

impl MelFilterBankNode {
    pub fn new(
        bands: usize,
        n_fft: u32,
        sample_rate: u32,
        min_frequency: f32,
        max_frequency: f32,
    ) -> Self {
        let filter_bank =
            MelFilterBank::init(sample_rate, n_fft, bands, min_frequency, max_frequency);

        Self {
            filter_bank,
        }
    }
}

impl DataHandler for MelFilterBankNode {
    fn handle(&self, port: usize, data: crate::nodes::Data) -> Vec<(usize, crate::nodes::Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            crate::nodes::Data::FloatArray(data) => {
                let data = self.filter_bank.filter_alloc(&data);
                vec![(0, crate::nodes::Data::FloatArray(data.into()))]
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
            0 => Some("Frequencies".into()),
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