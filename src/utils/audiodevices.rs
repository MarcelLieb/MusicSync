use std::collections::VecDeque;

use crate::utils::audioprocessing::{Buffer, ProcessingSettings};
use crate::utils::lights::LightService;
use cpal::traits::StreamTrait;
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
    BuildStreamError, StreamConfig,
};
use log::{debug, error, trace};

use crate::utils::audioprocessing::OnsetDetector;

pub fn create_monitor_stream(
    device_name: &str,
    processing_settings: ProcessingSettings,
    onset_detector: impl OnsetDetector + Send + 'static,
    lightservices: Vec<Box<dyn LightService + Send>>,
) -> Result<cpal::Stream, BuildStreamError> {
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
            d.name().unwrap_or_default().trim().to_lowercase() == device_name.trim().to_lowercase()
        })
        .ok_or(BuildStreamError::DeviceNotAvailable)?;

    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let channels = audio_cfg.channels();

    let config = StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(processing_settings.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut onset_detector = onset_detector;
    let mut lightservices = lightservices;

    let mut detection_buffer = Buffer::init(channels, &processing_settings);

    let buffer_size = processing_settings.buffer_size * channels as usize;
    let hop_size = processing_settings.hop_size * channels as usize;

    let mut buffer: VecDeque<f32> = VecDeque::new();

    let outstream = out.build_input_stream(
        &config,
        move |data: &[f32], _| {
            buffer.extend(data);
            let n = (buffer.len() + hop_size).saturating_sub(buffer_size) / hop_size;

            (0..n).for_each(|_| {
                detection_buffer.process_raw(&buffer.make_contiguous()[0..buffer_size]);
                trace!(
                    "RMS: {:.3}\t Peak: {:.3}",
                    detection_buffer.rms,
                    detection_buffer.peak
                );

                let onsets = onset_detector.detect(
                    &detection_buffer.freq_bins,
                    detection_buffer.peak,
                    detection_buffer.rms,
                );
                lightservices.process_onsets(&onsets);
                lightservices.process_spectrum(&detection_buffer.freq_bins);
                lightservices.process_samples(&detection_buffer.mono_samples);
                lightservices.update();

                buffer.drain(0..hop_size);
            })
        },
        |err| error!("an error occurred on stream: {}", err),
        None,
    );
    debug!("Default output device: {:?}", out.name().unwrap());
    debug!(
        "Default output sample format: {:?}",
        audio_cfg.sample_format()
    );
    debug!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    debug!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    debug!("Default output channels: {:?}", audio_cfg.channels());
    let stream = outstream?;
    stream
        .play()
        .map_err(|_| BuildStreamError::StreamConfigNotSupported)?;
    Ok(stream)
}

pub fn get_output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .unwrap()
        .filter_map(|d| d.name().ok())
        .collect()
}
