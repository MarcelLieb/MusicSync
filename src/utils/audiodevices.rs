use crate::utils::audioprocessing::hfc::Hfc;
use crate::utils::audioprocessing::spectral_flux::SpecFlux;
use crate::utils::audioprocessing::ProcessingSettings;
use crate::utils::lights::console::Console;
use crate::utils::lights::{hue, serialize};
use crate::utils::{
    audioprocessing::{prepare_buffers, process_raw},
    lights::LightService,
};
use cpal::{
    self,
    traits::{DeviceTrait, HostTrait},
    BuildStreamError, StreamConfig,
};
use log::debug;
use realfft::RealFftPlanner;

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

    let settings = ProcessingSettings::default();
    let config = StreamConfig {
        channels,
        sample_rate: cpal::SampleRate(settings.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(settings.fft_size);
    let mut detection_buffer = prepare_buffers(channels, &settings);

    let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();
    if let Ok(bridge) = hue::connect().await {
        lightservices.push(Box::new(bridge));
    }

    let console = Console::default();
    lightservices.push(Box::new(console));

    let serializer = serialize::OnsetContainer::init(
        "onsets.cbor".to_string(),
        settings.sample_rate as usize,
        settings.hop_size,
    );
    lightservices.push(Box::new(serializer));

    let mut spec_flux = SpecFlux::init(settings.sample_rate, settings.fft_size as u32);

    let mut _hfc = Hfc::init(settings.sample_rate as usize, settings.fft_size);

    let buffer_size = settings.buffer_size * channels as usize;
    let hop_size = settings.hop_size * channels as usize;
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

                        spec_flux.detect(
                            &detection_buffer.freq_bins,
                            peak,
                            rms,
                            &mut lightservices,
                        );
                        /*
                        _hfc.detect(
                            &detection_buffer.freq_bins,
                            peak,
                            rms,
                            &mut lightservices,
                        );
                         */

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
