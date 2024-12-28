use std::sync::Arc;

use log::warn;
use realfft::{RealFftPlanner, RealToComplex};

use crate::{nodes::{Data, DataHandler, DataType}, utils::audioprocessing::{window, WindowType}};



pub struct FFT {
    fft_planner: Arc<dyn RealToComplex<f32>>,
    fft_size: usize,
    window: Arc<[f32]>,
}

impl FFT {
    pub fn init(fft_size: usize, window_type: WindowType) -> Self {
        let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(fft_size as usize);
        let window = window(fft_size, window_type).into();
        Self {
            fft_planner,
            fft_size,
            window,
        }
    }
}

impl DataHandler for FFT {
    fn handle(&self, port: usize, data: Data) -> Vec<(usize, Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            Data::FloatArray(data) => {
                let mut data = data.to_vec();
                let data_len = data.len();
                if data_len < self.fft_size {
                    warn!("Data length is less than FFT size");
                    return vec![];
                }
                if data_len > self.fft_size {
                    warn!("Data length is greater than FFT size");
                    data.truncate(self.fft_size);
                }
                let mut output = self.fft_planner.make_output_vec();
                let mut data = data.into_iter().zip(self.window.iter()).map(|(a, b)| a * b).collect::<Vec<f32>>();
                self.fft_planner.process(&mut data, &mut output).unwrap();
                let data = output.iter().map(|x| x.norm()).collect::<Arc<[f32]>>();
                
                vec![(0, Data::FloatArray(data))]
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
            0 => Some(DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: usize) -> Option<DataType> {
        match port {
            0 => Some(DataType::FloatArray),
            _ => None,
        }
    }
}