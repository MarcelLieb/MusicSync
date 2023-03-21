use std::vec;

use cpal::{self, traits::{HostTrait, DeviceTrait}, Sample, StreamConfig};
use dasp_sample::ToSample;
use realfft::RealFftPlanner;
//use rustfft::num_complex::Complex;
//use rustfft::num_traits::Zero;
use log::debug;


fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

fn print_data<T>(data: &[T], f32_samples: &mut Vec<f32>)
where T: Sample + ToSample<f32> {
    f32_samples.clear();
    f32_samples.extend(data.iter().map(|x: &T| T::to_sample::<f32>(*x)));
    let buffer_size = f32_samples.len();

    f32_samples.extend(vec![0.0; f32_samples.capacity() - f32_samples.len()]);
    // println!("Frame length: {}", buffer_size);

    // Calculate RMS and peak volume
    let sound = f32_samples.iter().any(|i| *i != Sample::EQUILIBRIUM);
    let volume: f32 = (f32_samples
        .iter().fold(0.0, |acc, e| acc +  e * e) / buffer_size as f32).sqrt();
    let peak = f32_samples
        .iter().fold(0.0,|max, f| if f.abs() > max {f.abs()} else {max});
    if sound {
        println!("RMS: {:.3}, Peak: {:.3}", volume, peak);
    }


    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(f32_samples.capacity());

    let mut output = fft.make_output_vec();
    match fft.process(f32_samples, &mut output) {
        Ok(_) => (),
        Err(e) => println!("Error: {:?}", e)
    }

    let output = output.iter().map(|e| e.re.abs()).collect::<Vec<f32>>();

}

pub fn create_default_output_stream() -> cpal::Stream {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let out = default_host.default_output_device().expect("no output device available");
    let audio_cfg = out
        .default_output_config()
        .expect("No default output config found");

    let mut f32_samples: Vec<f32> = Vec::with_capacity(22050);
    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match out.build_input_stream(
            &audio_cfg.config(),
            move |data: &[f32], _| print_data(data, &mut f32_samples),
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
                move |data: &[i16], _| print_data(data, &mut f32_samples),
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
                move |data: &[u16], _| print_data(data, &mut f32_samples),
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

fn split_channels<T> (config: &StreamConfig, data: &[T]) -> Vec<Vec<f32>> 
where T: Sample + ToSample<f32> {
    let samples: Vec<f32> = data.iter().map(|s| s.to_sample::<f32>()).collect();
    let channels = config.channels;
    let mut out: Vec<Vec<f32>> = Vec::new();
    for i in 0..channels {
        out.push(samples.iter().enumerate().filter_map(|(index, f)| if index as u16 % channels == i {Some(*f)} else {None}).collect())
    }
    out
}