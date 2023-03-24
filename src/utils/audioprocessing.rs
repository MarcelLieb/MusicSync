use std::collections::VecDeque;

use cpal::Sample;
use dasp_sample::ToSample;
use realfft::RealFftPlanner;
use log::info;

pub fn print_data<T>(data: &[T], channels: u16, f32_samples: &mut Vec<Vec<f32>>, threshold: &mut DynamicThreshold)
where T: Sample + ToSample<f32> {
    split_channels(channels, data, f32_samples);

    // Pad with trailing zeros
    for channel in f32_samples.iter_mut() {
        channel.extend(vec![0.0; channel.capacity() - channel.len()])
    }

    // Check for silence
    let sound = f32_samples[0]
        .iter()
        .any(|i| *i != Sample::EQUILIBRIUM);

    if sound {
        let volume: Vec<f32> = f32_samples.iter()
            .map(|c| (c.iter()
                .fold(0.0, |acc, e| acc +  e * e) / c.len() as f32)
                .sqrt())
            .collect();

        let peak = f32_samples
            .iter()
            .map(|c| c.iter()
                .fold(0.0,|max, f| if f.abs() > max {f.abs()} else {max})
            )
            .reduce(f32::max).unwrap();

        info!("RMS: {:.3}, Peak: {:.3}", volume.iter().sum::<f32>() / volume.len() as f32, peak);

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(f32_samples[0].capacity());

        let mut output = fft.make_output_vec();
        match fft.process(&mut f32_samples[0], &mut output) {
            Ok(()) => (),
            Err(e) => println!("Error: {:?}", e)
        }

        let output = output
            .iter()
            .map(|e| (e.re * e.re + e.im * e.im).sqrt())
            .collect::<Vec<f32>>();

        let weighted: Vec<f32> = output
            .iter()
            .enumerate()
            .map(|(k, freq)| k as f32 * freq)
            .collect();

        let weight: f32 = weighted.iter().sum();

        if weight >= threshold.get_threshold(weight) {
            println!("Onset!");
        }
        else {
            println!();
        }

        let index_of_max = output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
            .unwrap(); 

        info!("Loudest frequency: {}Hz", index_of_max);
    }
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

#[derive(Debug, Clone)]
pub struct DynamicThreshold {
    past_thresholds: VecDeque<f32>,
    buffer_size: usize
}

#[allow(dead_code)]
impl DynamicThreshold {
    pub fn init() -> Self {
        DynamicThreshold { 
            past_thresholds: VecDeque::with_capacity(3), buffer_size: 3
        }
    }

    pub fn init_buffer(buffer_size: usize) -> Self {
        DynamicThreshold { 
            past_thresholds: VecDeque::with_capacity(buffer_size), buffer_size: buffer_size
        }
    }

    fn get_threshold(&mut self, value: f32) -> f32 {
        let sum: f32 = self.past_thresholds.iter().sum();
        if self.past_thresholds.len() >= self.buffer_size {
            self.past_thresholds.pop_front();
            self.past_thresholds.push_back(value);
        }
        else {
            self.past_thresholds.push_back(value)
        }
        sum / self.past_thresholds.len() as f32
    }


}