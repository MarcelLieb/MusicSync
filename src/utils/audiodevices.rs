use cpal::{self, traits::{HostTrait, DeviceTrait}, BuildStreamError, StreamConfig};
use log::{debug};
use realfft::{RealFftPlanner, num_complex::Complex};
use crate::utils::{audioprocessing::{print_onset, DynamicThreshold, MultiBandThreshold}, hue::Bridge, lights::LightService};


pub const SAMPLE_RATE: u32 = 48000;
// buffer size is 10 ms long
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
    let config = StreamConfig {
        channels: channels,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE)
    };

    let mut f32_samples: Vec<Vec<f32>> = Vec::with_capacity(channels.into());
    for _ in 0..channels {
        f32_samples.push(Vec::with_capacity(SAMPLE_RATE as usize));
    }
    let mut mono_samples: Vec<f32> = Vec::with_capacity(SAMPLE_RATE as usize);

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(SAMPLE_RATE as usize);
    let mut fft_output: Vec<Vec<Complex<f32>>> = (0..channels).map(|_| fft_planner.make_output_vec()).collect();
    let mut freq_bins: Vec<f32> = vec![0.0; fft_output[0].capacity()];

    let mut multi_threshold = MultiBandThreshold {
        drums: DynamicThreshold::init_config(18, Some(0.22), Some(0.20)),
        hihat: DynamicThreshold::init_config(20, Some(0.32), Some(0.23)),
        notes: DynamicThreshold::init_config(20, None, Some(0.20)),
        fullband: DynamicThreshold::init_config(20, None, None),
    };

    let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();
    let bridge = Box::new(Bridge::init().unwrap());
    lightservices.push(bridge);

    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => out.build_input_stream(
            &config,
            move |data: &[f32], _| print_onset(data, channels, &mut f32_samples, &mut mono_samples, &fft_planner, &mut fft_output, &mut freq_bins, &mut multi_threshold, &mut lightservices),
            capture_err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => {
            out.build_input_stream(
                &config,
                move |data: &[i16], _| print_onset(data, channels, &mut f32_samples, &mut mono_samples, &fft_planner, &mut fft_output, &mut freq_bins, &mut multi_threshold, &mut lightservices),
                capture_err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            out.build_input_stream(
                &config,
                move |data: &[u16], _| print_onset(data, channels, &mut f32_samples, &mut mono_samples, &fft_planner, &mut fft_output, &mut freq_bins, &mut multi_threshold, &mut lightservices),
                capture_err_fn,
                None,
            )
        }
        _ => Err(BuildStreamError::StreamConfigNotSupported)
    }.expect("Couldn't build input stream.\nMake sure you are running at 48kHz sample rate");
    debug!("Default output device: {:?}", out.name().unwrap());
    debug!("Default output sample format: {:?}", audio_cfg.sample_format());
    debug!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    debug!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    debug!("Default output channels: {:?}", audio_cfg.channels());
    outstream
}