use cpal::{self, traits::{HostTrait, DeviceTrait, StreamTrait}, Sample};
use dasp_sample::ToSample;

fn main() {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let out = default_host.default_output_device().expect("no output device available");
    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let _outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match out.build_input_stream(
            &audio_cfg.config(),
            move |data, _: &_| print_data::<f32>(data),
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
                move |data, _: &_| print_data::<i16>(data),
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
                move |data, _: &_| print_data::<u16>(data),
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
    } .unwrap();
    _outstream.play().unwrap();
    println!("Default output device: {:?}", out.name().unwrap());
    println!("Default output sample format: {:?}", audio_cfg.sample_format());
    println!("Default output buffer size: {:?}", audio_cfg.buffer_size());
    println!("Default output sample rate: {:?}", audio_cfg.sample_rate());
    println!("Default output channels: {:?}", audio_cfg.channels());
    //println!("Stream was created: {}", outstream.is_some());
    std::thread::sleep(std::time::Duration::from_millis(10000));

    println!("Stream was dropped");
}

fn print_data<T>(data: &[T])
where T: Sample + ToSample<f32> {
    println!("Frame length: {}", data.len());
    let sound = data.iter().any(|i| *i != Sample::EQUILIBRIUM);
    if sound {
        println!("Sound!");
    }
}

fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}