use cpal::{self, traits::{HostTrait, DeviceTrait}, Sample};
use dasp_sample::ToSample;
use log::debug;


fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

fn print_data<T>(data: &[T])
where T: Sample + ToSample<f32> {
    //println!("Frame length: {}", data.len());
    let sound = data.iter().any(|i| *i != Sample::EQUILIBRIUM);
    let volume: f32 = data
        .iter().fold(0.0, |acc, e:&T| acc +  T::to_sample::<f32>(*e).abs()) / data.len() as f32;
    let peak = data
        .iter().map(|s| T::to_sample::<f32>(*s))
        .into_iter().fold(-1.0,|max, f| if f > max {f} else {max})
        .abs();
    if sound {
        println!("RMS: {:.3}, Peak: {:.3}", volume, peak);
    }
}

pub fn create_default_output_stream() -> cpal::Stream {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let out = default_host.default_output_device().expect("no output device available");
    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match out.build_input_stream(
            &audio_cfg.config(),
            move |data: &[f32], _| print_data(data),
            capture_err_fn,
            None,
        ) {
            Ok(stream) => Some(stream),
            Err(e) => {
                panic!("{:?}", e)
            }
        },
        cpal::SampleFormat::I16 => {
            match out.build_input_stream(
                &audio_cfg.config(),
                move |data: &[i16], _| print_data(data),
                capture_err_fn,
                None,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    panic!("{:?}", e)
                }
            }
        }
        cpal::SampleFormat::U16 => {
            match out.build_input_stream(
                &audio_cfg.config(),
                move |data: &[u16], _| print_data(data),
                capture_err_fn,
                None,
            ) {
                Ok(stream) => Some(stream),
                Err(e) => {
                    panic!("{:?}", e)
                }
            }
        }
        _ => None,
    };
    debug!("Default output device: {:?}", out.name().unwrap());
    debug!("Default output sample format: {:?}", audio_cfg.sample_format());
    debug!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    debug!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    debug!("Default output channels: {:?}", audio_cfg.channels());
    outstream.unwrap()
}