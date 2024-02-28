use std::collections::VecDeque;

use crate::utils::audioprocessing::spectral_flux::SpecFlux;
use crate::utils::audioprocessing::ProcessingSettings;
use crate::utils::audioprocessing::{prepare_buffers, process_raw};
use crate::utils::lights::console::Console;
use crate::utils::lights::{hue, serialize, wled, LightService};
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
    BuildStreamError, StreamConfig,
};
use log::debug;

use super::audioprocessing::spectral_flux::SpecFluxSettings;

fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

pub fn create_monitor_stream(
    device_name: &str,
    processing_settings: ProcessingSettings,
    detection_settings: SpecFluxSettings,
    lightservices: Vec<Box<dyn LightService + Send>>,
) -> Result<cpal::Stream, BuildStreamError> {
    let out = cpal::default_host()
        .devices()
        .map_err(|_| BuildStreamError::DeviceNotAvailable)?
        .filter(|d| d.name().unwrap_or_default() == device_name)
        .next()
        .ok_or_else(|| BuildStreamError::DeviceNotAvailable)?;

    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let channels = audio_cfg.channels();

    let config = StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(processing_settings.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut lightservices = lightservices;

    let mut detection_buffer = prepare_buffers(channels, &processing_settings);

    let mut spec_flux = SpecFlux::with_settings(
        processing_settings.sample_rate,
        processing_settings.fft_size as u32,
        detection_settings,
    );

    let buffer_size = processing_settings.buffer_size * channels as usize;
    let hop_size = processing_settings.hop_size * channels as usize;
    macro_rules! build_buffered_onset_stream {
        ($t:ty) => {{
            let mut buffer: VecDeque<$t> = VecDeque::new();

            out.build_input_stream(
                &config,
                move |data: &[$t], _| {
                    buffer.extend(data);
                    let n = (buffer.len() + hop_size).saturating_sub(buffer_size) / hop_size;

                    (0..n).for_each(|_| {
                        process_raw(
                            &buffer.make_contiguous()[0..buffer_size],
                            channels,
                            &mut detection_buffer,
                        );

                        let onsets = spec_flux.detect(
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
                capture_err_fn,
                None,
            )
        }};
    }
    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => build_buffered_onset_stream!(f32),
        cpal::SampleFormat::I16 => build_buffered_onset_stream!(i16),
        cpal::SampleFormat::U16 => build_buffered_onset_stream!(u16),
        _ => Err(BuildStreamError::StreamConfigNotSupported),
    };
    debug!("Default output device: {:?}", out.name().unwrap());
    debug!(
        "Default output sample format: {:?}",
        audio_cfg.sample_format()
    );
    debug!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    debug!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    debug!("Default output channels: {:?}", audio_cfg.channels());
    outstream
}

pub async fn create_default_output_stream() -> Result<cpal::Stream, BuildStreamError> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or_else(|| BuildStreamError::DeviceNotAvailable)?;

    let settings = ProcessingSettings::default();

    let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();
    match hue::connect().await {
        Ok(bridge) => lightservices.push(Box::new(bridge)),
        Err(e) => println!("{e}"),
    }

    /*
    if let Ok(strip) = wled::LEDStripOnset::connect("192.168.2.57").await {
        lightservices.push(Box::new(strip));
    }
     */
    match wled::LEDStripSpectrum::connect("192.168.2.57", settings.sample_rate as f32).await {
        Ok(strip) => lightservices.push(Box::new(strip)),
        Err(e) => println!("{e}"),
    }

    match wled::LEDStripSpectrum::connect("192.168.2.58", settings.sample_rate as f32).await {
        Ok(strip) => lightservices.push(Box::new(strip)),
        Err(e) => println!("{e}"),
    }

    let console = Console::default();
    lightservices.push(Box::new(console));

    let serializer = serialize::OnsetContainer::init(
        "onsets.cbor",
        settings.sample_rate as usize,
        settings.hop_size,
    );
    lightservices.push(Box::new(serializer));
    let detection_settings = SpecFluxSettings::default();
    create_monitor_stream(
        &device.name().unwrap_or_default(),
        settings,
        detection_settings,
        lightservices,
    )
}
