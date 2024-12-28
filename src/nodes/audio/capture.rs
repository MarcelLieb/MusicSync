use std::sync::Arc;

use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BuildStreamError, StreamConfig,
};
use log::error;

use crate::nodes::{DataGraphManager, DataHandler, DataType};

pub struct LoopbackNode {
    pub id: Arc<str>,
    channels: u16,
    stream: cpal::Stream,
}

impl LoopbackNode {
    pub fn new(manager: &DataGraphManager, device_name: &str, sample_rate: u32) -> Result<Self, BuildStreamError> {
        let device_name = if device_name.trim().is_empty() {
            cpal::default_host()
                .default_output_device()
                .ok_or(BuildStreamError::DeviceNotAvailable)?
                .name()
                .map_err(|_| BuildStreamError::DeviceNotAvailable)?
        } else {
            device_name.to_owned()
        };

        let out = cpal::default_host()
            .devices()
            .map_err(|_| BuildStreamError::DeviceNotAvailable)?
            .find(|d| {
                d.name().unwrap_or_default().trim().to_lowercase()
                    == device_name.trim().to_lowercase()
            })
            .ok_or(BuildStreamError::DeviceNotAvailable)?;

            let audio_cfg = out
            .default_output_config()
            .expect("No default output config found");
    
        let channels = audio_cfg.channels();
    
        let config = StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let id = manager.add_reference(1);
        let tx = manager.get_input_queue();
        let id_inner = id.clone();

        let stream = out.build_input_stream(
            &config,
            move |data: &[f32], _| {
                let data = data.to_vec();
                if data.iter().all(|&x| x == 0.0) {
                    return;
                }
                tx.send(((id_inner.clone(), 0), crate::nodes::Data::FloatArray(data.into()))).unwrap();
            },
            move |err| {
                error!("An error occurred on the output audio stream: {}", err);
            },
            None
        )?;
        stream.play().unwrap();

        Ok(Self { id, stream, channels })
    }
}

impl DataHandler for LoopbackNode {
    fn handle(
        &mut self,
        _: usize,
        _: crate::nodes::Data,
    ) -> Vec<(usize, crate::nodes::Data)> {
        vec![]
    }

    fn num_input_ports(&self) -> usize {
        0
    }

    fn num_output_ports(&self) -> usize {
        self.channels as usize
    }

    fn get_input_name(&self, _: usize) -> Option<Arc<str>> {
        None
    }

    fn get_output_name(&self, port: usize) -> Option<Arc<str>> {
        Some(format!("Channel {}", port).into())
    }
    
    fn get_input_type(&self, _: usize) -> Option<crate::nodes::DataType> {
        None
    }
    
    fn get_output_type(&self, port: usize) -> Option<crate::nodes::DataType> {
        if port < self.channels as usize {
            Some(crate::nodes::DataType::FloatArray)
        } else {
            None
        }
    }
}
