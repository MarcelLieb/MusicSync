use crate::utils::{
    audioprocessing::{prepare_buffers, process_raw, threshold::MultiBandThreshold},
    hue,
    lights::{Console, LightService},
    serialize,
};
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
    BuildStreamError, StreamConfig,
};
use crate::utils::audioprocessing::hfc::hfc;
use log::debug;
use realfft::RealFftPlanner;

pub const SAMPLE_RATE: u32 = 48000;
// buffer size is 10 ms long
pub const BUFFER_SIZE: u32 = 1024;
pub const HOP_SIZE: u32 = 480;

fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

pub async fn create_default_output_stream() -> cpal::Stream {
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
        buffer_size: cpal::BufferSize::Default,
    };

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(SAMPLE_RATE as usize);
    let mut detection_buffer = prepare_buffers(channels, SAMPLE_RATE);

    let mut multi_threshold = MultiBandThreshold::default();

    let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();
    if let Ok(bridge) = hue::connect().await {
        lightservices.push(Box::new(bridge));
    }

    let console = Console::default();
    lightservices.push(Box::new(console));

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
                        let (peak, rms) = process_raw(
                            &buffer[0..buffer_size],
                            channels,
                            &fft_planner,
                            &mut detection_buffer,
                        );

                        hfc(
                            &detection_buffer.freq_bins, 
                            peak, 
                            rms,
                            &mut multi_threshold, 
                            None, 
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
