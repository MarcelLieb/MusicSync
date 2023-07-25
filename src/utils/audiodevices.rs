use crate::utils::{
    audioprocessing::{print_onset, DynamicThreshold, MultiBandThreshold},
    hue::Bridge,
    lights::LightService,
    serialize,
};
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
    BuildStreamError, StreamConfig,
};
use log::debug;
use realfft::{num_complex::Complex, RealFftPlanner};

pub const SAMPLE_RATE: u32 = 48000;
// buffer size is 10 ms long
pub const BUFFER_SIZE: u32 = 480;
pub const HOP_SIZE: u32 = BUFFER_SIZE / 3 * 2;

fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

pub fn create_default_output_stream() -> cpal::Stream {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();

    let out = default_host
        .default_output_device()
        .expect("no output device available");
    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let channels = audio_cfg.channels();
    let config = StreamConfig {
        channels: channels,
        sample_rate: cpal::SampleRate(SAMPLE_RATE),
        buffer_size: cpal::BufferSize::Fixed(BUFFER_SIZE),
    };

    let mut f32_samples: Vec<Vec<f32>> = Vec::with_capacity(channels.into());
    for _ in 0..channels {
        f32_samples.push(Vec::with_capacity(SAMPLE_RATE as usize));
    }
    let mut mono_samples: Vec<f32> = Vec::with_capacity(SAMPLE_RATE as usize);

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(SAMPLE_RATE as usize);
    let mut fft_output: Vec<Vec<Complex<f32>>> = (0..channels)
        .map(|_| fft_planner.make_output_vec())
        .collect();
    let mut freq_bins: Vec<f32> = vec![0.0; fft_output[0].capacity()];

    let mut multi_threshold = MultiBandThreshold {
        drums: DynamicThreshold::init_config(30, Some(0.30), Some(0.18)),
        hihat: DynamicThreshold::init_config(20, Some(0.30), Some(0.15)),
        notes: DynamicThreshold::init_config(20, None, None),
        fullband: DynamicThreshold::init_config(20, None, None),
    };

    let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();
    if let Ok(bridge) = Bridge::init() {
        lightservices.push(Box::new(bridge));
    }

    let serializer = serialize::OnsetContainer::init("onsets.cbor".to_string());
    lightservices.push(Box::new(serializer));

    let buffer_size = (BUFFER_SIZE * channels as u32) as usize;
    let hop_size = (HOP_SIZE * channels as u32) as usize;
    macro_rules! build_buffered_onset_stream {
        ($t:ty) => {{
            let mut buffer: Vec<$t> = Vec::new();

            out.build_input_stream(
                &config,
                move |data: &[$t], _| {
                    buffer.extend(data);
                    let n = (buffer.len() + hop_size).saturating_sub(buffer_size) / hop_size;

                    (0..n).for_each(|_| {
                        print_onset(
                            &buffer[0..buffer_size],
                            channels,
                            &mut f32_samples,
                            &mut mono_samples,
                            &fft_planner,
                            &mut fft_output,
                            &mut freq_bins,
                            &mut multi_threshold,
                            &mut lightservices,
                        );

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
    }
    .expect("Couldn't build input stream.\nMake sure you are running at 48kHz sample rate");
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
