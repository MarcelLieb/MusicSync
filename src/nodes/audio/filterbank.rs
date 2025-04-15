use std::sync::{Arc, Mutex};

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Q_BUTTERWORTH_F32};
use log::warn;

use crate::{
    nodes::{self, Data, DataHandler, PortId}, utils::audioprocessing::MelFilterBank
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
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
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
            0 => Some("Frequencies".into()),
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
            0 => Some(crate::nodes::DataType::FloatArray),
            _ => None,
        }
    }
    
}

pub struct ThreeBandFilter {
    low_pass: Mutex<DirectForm2Transposed<f32>>,
    high_pass: Mutex<DirectForm2Transposed<f32>>,
}

impl ThreeBandFilter {
    pub fn new(low_crossover: f64, high_crossover: f64, sample_rate: usize) -> Self {
        Self {
            low_pass: Mutex::new(
                DirectForm2Transposed::new(
                    Coefficients::<f32>::from_params(
                        biquad::Type::LowPass, sample_rate.hz(), low_crossover.hz(), Q_BUTTERWORTH_F32
                    ).unwrap()
                )
            ), 
            high_pass: Mutex::new(
                DirectForm2Transposed::new(
                    Coefficients::<f32>::from_params(
                        biquad::Type::LowPass, sample_rate.hz(), high_crossover.hz(), Q_BUTTERWORTH_F32
                    ).unwrap()
                )
            )
        }
    }
}

impl DataHandler for ThreeBandFilter {
    fn handle(&self, port: PortId, data: crate::nodes::Data) -> Vec<(PortId, crate::nodes::Data)> {
        if port != 0 {
            return vec![];
        }

        match data {
            Data::Float(v) => {
                let low = self.low_pass.lock().unwrap().run(v);
                let high = self.high_pass.lock().unwrap().run(v);
                let mid = v - low - high;
                return vec![(0, Data::Float(low)), (1, Data::Float(mid)), (2, Data::Float(high))];
            }
            _ => return vec![]
        }
    }

    fn num_input_ports(&self) -> PortId {
        1
    }

    fn num_output_ports(&self) -> PortId {
        3
    }

    fn get_input_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        if port == 0 {
            return Some(nodes::DataType::Float)
        }
        None
    }

    fn get_output_type(&self, port: PortId) -> Option<crate::nodes::DataType> {
        match port {
            0 | 1 | 2 => Some(nodes::DataType::Float),
            _ => None
        }
    }

    fn get_input_name(&self, port: PortId) -> Option<Arc<str>> {
        if port == 0 {
            return Some("Signal".into())
        }
        None
    }

    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("low".into()),
            1 => Some("mid".into()),
            2 => Some("high".into()),
            _ => None
        }
    }
}