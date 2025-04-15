use std::sync::Arc;

use cpal::{
    self,
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BuildStreamError, StreamConfig,
};
use log::error;

use crate::nodes::{DataGraphManager, DataHandler, DataType, NodeId, PortId};

pub struct LoopbackNode {
    pub id: NodeId,
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

        let id = manager.add_reference(channels as PortId);
        let tx = manager.get_input_queue();

        let stream = out.build_input_stream(
            &config,
            move |data: &[f32], _| {
                let data = data.to_vec();
                if data.iter().all(|&x| x == 0.0) {
                    return;
                }
                let audio = (0..channels).map(|i| {
                    data.iter().enumerate().filter_map(|(j, &x)| {
                        if j % channels as usize == i as usize {
                            Some(x)
                        } else {
                            None
                        }
                    }).collect::<Arc<[f32]>>()
                }).collect::<Vec<_>>();
                for (i, data) in audio.into_iter().enumerate() {
                    tx.send(((id, i as PortId), crate::nodes::Data::FloatArray(data))).unwrap();
                }
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
        &self,
        _: PortId,
        _: crate::nodes::Data,
    ) -> Vec<(PortId, crate::nodes::Data)> {
        vec![]
    }

    fn num_input_ports(&self) -> PortId {
        0
    }

    fn num_output_ports(&self) -> PortId {
        self.channels as PortId
    }

    fn get_input_name(&self, _: PortId) -> Option<Arc<str>> {
        None
    }

    fn get_output_name(&self, port: PortId) -> Option<Arc<str>> {
        if port > self.channels as PortId {
            return None;
        }
        Some(format!("Channel {}", port).into())
    }
    
    fn get_input_type(&self, _: PortId) -> Option<DataType> {
        None
    }
    
    fn get_output_type(&self, port: PortId) -> Option<DataType> {
        if port < self.channels as PortId {
            Some(crate::nodes::DataType::FloatArray)
        } else {
            None
        }
    }
}
