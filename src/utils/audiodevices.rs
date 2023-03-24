use cpal::{self, traits::{HostTrait, DeviceTrait}, BuildStreamError};
use log::debug;
use crate::utils::audioprocessing::print_data;


fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

pub fn create_default_output_stream() -> cpal::Stream {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let out = default_host.default_output_device().expect("no output device available");
    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let channels = audio_cfg.channels();
    let mut f32_samples: Vec<Vec<f32>> = Vec::with_capacity(channels.into());
    for _ in 0..channels {
        f32_samples.push(Vec::with_capacity(44100));
    }
    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => out.build_input_stream(
            &audio_cfg.config(),
            move |data: &[f32], _| print_data(data, channels, &mut f32_samples),
            capture_err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => {
            out.build_input_stream(
                &audio_cfg.config(),
                move |data: &[i16], _| print_data(data, channels, &mut f32_samples),
                capture_err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            out.build_input_stream(
                &audio_cfg.config(),
                move |data: &[u16], _| print_data(data, channels, &mut f32_samples),
                capture_err_fn,
                None,
            )
        }
        _ => Err(BuildStreamError::StreamConfigNotSupported)
    }.ok().unwrap();
    debug!("Default output device: {:?}", out.name().unwrap());
    debug!("Default output sample format: {:?}", audio_cfg.sample_format());
    debug!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    debug!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    debug!("Default output channels: {:?}", audio_cfg.channels());
    outstream
}