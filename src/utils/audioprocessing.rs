use std::{collections::VecDeque, f32::consts::PI};

use cpal::Sample;
use dasp_sample::ToSample;
use realfft::{RealFftPlanner, num_traits::SaturatingMul};
use log::info;

pub fn print_data<T>(data: &[T], channels: u16, f32_samples: &mut Vec<Vec<f32>>, threshold: &mut DynamicThreshold)
where T: Sample + ToSample<f32> {
    split_channels(channels, data, f32_samples);

    window(f32_samples);
    // Pad with trailing zeros
    f32_samples
        .iter_mut()
        .for_each(|channel| 
            channel
                .extend(vec![0.0; channel.capacity() - channel.len()]));

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

        let mut input = f32_samples
            .iter()
            .fold(vec![0.0; f32_samples[0].len()],|sum: Vec<f32>, channel: &Vec<f32>|
                sum
                    .iter()
                    .zip(channel)
                    .map(|(s, c)| *s + c)
                    .collect::<Vec<f32>>()
            );

        let mut output = fft.make_output_vec();
        match fft.process(&mut input, &mut output) {
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
        let mut sum: f32 = self.past_thresholds.iter().sum();
        if self.past_thresholds.len() >= self.buffer_size {
            self.past_thresholds.pop_front();
            self.past_thresholds.push_back(value);
        }
        else {
            self.past_thresholds.push_back(value);
            sum = f32::MAX;
        }
        sum / self.past_thresholds.len() as f32
    }
}

#[allow(unused_variables, non_snake_case)]
fn window(samples: &mut Vec<Vec<f32>>) {
    let N = samples[0].len();
    //Hann window
    let window_Hann: Vec<f32> = (0..N)
        .map(|n| 0.5 * (1. - f32::cos(2. * PI * n as f32 / N as f32)))
        .collect();

    // Matlab coefficents from wikipedia
    const A: [f32; 5] = [0.21557895, 0.41663158, 0.277263158, 0.083578947, 0.006947368]; 
    let window_flat_top: Vec<f32> = (0..N)
        .map(|n| 
            A[0] 
            - A[1] * (2. * PI * n as f32 / N as f32).cos() 
            + A[2] * (4. * PI * n as f32 / N as f32).cos() 
            - A[3] * (6. * PI * n as f32 / N as f32).cos() 
            + A[4] * (8. * PI * n as f32 / N as f32).cos()
        )
        .collect();

    // Apply window
    samples
        .iter_mut()
        .for_each(|channel|
            channel
                .iter_mut()
                .zip(window_Hann.iter())
                .map(|(x, w)| *x = *x * w)
                .for_each(drop)
        );
}