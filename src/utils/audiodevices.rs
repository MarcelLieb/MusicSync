use cpal::{self, traits::{HostTrait, DeviceTrait}, BuildStreamError, StreamConfig};
use log::{debug};
use crate::utils::audioprocessing::{print_onset, DynamicThreshold};


pub const SAMPLE_RATE: u32 = 48000;
pub const BUFFER_SIZE: u32 = 480;

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
        f32_samples.push(Vec::with_capacity(audio_cfg.sample_rate().0 as usize));
    }
    // buffer size is aprox. 10 ms long
    let config = StreamConfig {
        channels: channels,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE)
    };
    let mut threshold = DynamicThreshold::init_buffer(20);
    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => out.build_input_stream(
            &config,
            move |data: &[f32], _| print_onset(data, channels, &mut f32_samples, &mut threshold),
            capture_err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => {
            out.build_input_stream(
                &config,
                move |data: &[i16], _| print_onset(data, channels, &mut f32_samples, &mut threshold),
                capture_err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            out.build_input_stream(
                &config,
                move |data: &[u16], _| print_onset(data, channels, &mut f32_samples, &mut threshold),
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