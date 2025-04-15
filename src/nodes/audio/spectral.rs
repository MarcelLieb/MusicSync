use std::sync::{Arc, RwLock};

use log::warn;
use realfft::{RealFftPlanner, RealToComplex};

use crate::{nodes::{Data, DataHandler, DataType, PortId}, utils::audioprocessing::{window, WindowType}};



pub struct FFT {
    fft_planner: Arc<dyn RealToComplex<f32>>,
    fft_size: usize,
    window: RwLock<Arc<[f32]>>,
    window_type: WindowType,
}

impl FFT {
    pub fn init(fft_size: usize, window_type: WindowType) -> Self {
        let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(fft_size as usize);
        let window = RwLock::new(window(fft_size, window_type).into());
        Self {
            fft_planner,
            fft_size,
            window,
            window_type,
        }
    }
}

impl DataHandler for FFT {
    fn handle(&self, port: PortId, data: Data) -> Vec<(PortId, Data)> {
        if port != 0 {
            warn!("Invalid port");
            return vec![];
        }
        match data {
            Data::FloatArray(data) => {
                let mut data = data;
                let data_len = data.len();
                let window_len = self.window.read().unwrap().len();
                if window_len != data_len && window_len <= self.fft_size {
                    let mut window_ = self.window.write().unwrap();
                    *window_ = window(data_len, self.window_type).into();
                }
                let pointer = Arc::make_mut(&mut data);
                pointer.iter_mut().zip(self.window.read().unwrap().iter()).for_each(|(a, b)| *a = *a * b);
                let mut data: Vec<f32> = Vec::<f32>::from(&*data);
                if data_len < self.fft_size {
                    data.resize(self.fft_size, 0.0);
                }
                if data_len > self.fft_size {
                    warn!("Data length is greater than FFT size");
                    data.truncate(self.fft_size);
                }
                let mut output = self.fft_planner.make_output_vec();
                let n = self.fft_size;
                self.fft_planner.process(&mut data, &mut output).unwrap();
                let data = output.iter().map(|x| ((x.re * x.re + x.im * x.im) / n as f32).sqrt()).collect::<Arc<[f32]>>();
                
                vec![(0, Data::FloatArray(data))]
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
            0 => Some("Signal".into()),
            _ => None,
        }
    }

    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        match port {
            0 => Some("FFT".into()),
            _ => None,
        }
    }

    fn get_input_type(&self, port: PortId) -> Option<DataType> {
        match port {
            0 => Some(DataType::FloatArray),
            _ => None,
        }
    }

    fn get_output_type(&self, port: PortId) -> Option<DataType> {
        match port {
            0 => Some(DataType::FloatArray),
            _ => None,
        }
    }
}