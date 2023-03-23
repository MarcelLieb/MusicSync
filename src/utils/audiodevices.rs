use cpal::{self, traits::{HostTrait, DeviceTrait}, Sample};
use dasp_sample::ToSample;
use realfft::RealFftPlanner;
use log::debug;


fn capture_err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

fn print_data<T>(data: &[T], channels: u16, f32_samples: &mut Vec<Vec<f32>>)
where T: Sample + ToSample<f32> {
    split_channels(channels, data, f32_samples);
    let buffer_size = f32_samples.len();

    // Pad with trailing zeros
    for channel in f32_samples.iter_mut() {
        channel.extend(vec![0.0; channel.capacity() - channel.len()])
    }

    // println!("Frame length: {}", buffer_size);

    // Calculate RMS and peak volume
    let sound = f32_samples[0]
        .iter()
        .any(|i| *i != Sample::EQUILIBRIUM);

    let volume: Vec<f32> = f32_samples.iter()
        .map(|c| (c.iter()
            .fold(0.0, |acc, e| acc +  e * e) / buffer_size as f32)
            .sqrt())
        .collect();

    let peak = f32_samples
        .iter()
        .map(|c| c.iter()
            .fold(0.0,|max, f| if f.abs() > max {f.abs()} else {max})
        )
        .reduce(f32::max).unwrap();

    if sound {
        println!("RMS: {:.3}, Peak: {:.3}", volume.iter().sum::<f32>() / volume.len() as f32, peak);
    }


    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(f32_samples[0].capacity());

    let mut output = fft.make_output_vec();
    match fft.process(&mut f32_samples[0], &mut output) {
        Ok(_) => (),
        Err(e) => println!("Error: {:?}", e)
    }

    let output = output
        .iter()
        .map(|e| e.re.abs())
        .collect::<Vec<f32>>();

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
        f32_samples.push(Vec::with_capacity(11025));
    }
    let outstream = match audio_cfg.sample_format() {
        cpal::SampleFormat::F32 => match out.build_input_stream(
            &audio_cfg.config(),
            move |data: &[f32], _| print_data(data, channels, &mut f32_samples),
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
                move |data: &[i16], _| print_data(data, channels, &mut f32_samples),
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
                move |data: &[u16], _| print_data(data, channels, &mut f32_samples),
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

fn split_channels<T> (channels: u16, data: &[T], f32_samples: &mut Vec<Vec<f32>>) 
where T: Sample + ToSample<f32> {
    for (i, channel) in f32_samples.iter_mut().enumerate() {
        channel.clear();
        channel.extend(
            data.iter()
            .map(|s| s.to_sample::<f32>())
            .enumerate()
            .filter_map(|(index, f)| if index % channels as usize == i {Some(f)} else {None})
        );
    }
}