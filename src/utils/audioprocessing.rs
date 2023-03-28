use std::{collections::VecDeque, f32::consts::PI, sync::Arc};

use cpal::Sample;
use dasp_sample::ToSample;
use realfft::{RealToComplex, num_complex::{Complex32}};
use log::info;
use colored::Colorize;
use lazy_static::lazy_static;

use crate::utils::audiodevices;

lazy_static! {
    static ref FFT_WINDOW: Vec<f32> = window(audiodevices::BUFFER_SIZE as usize, WindowType::Hann);
    static ref THRESHOLD_WINDOW: Vec<f32> = window(39, WindowType::Hann);
}

pub fn print_onset<T>(
    data: &[T], 
    channels: u16,
    f32_samples: &mut Vec<Vec<f32>>, 
    mono_samples: &mut Vec<f32>,
    fft_planner: &Arc<dyn RealToComplex<f32>>,
    fft_output: &mut Vec<Complex32>,
    freq_bins: &mut Vec<f32>,
    threshold: &mut DynamicThreshold
)
where T: Sample + ToSample<f32> {
    split_channels(channels, data, f32_samples);

    apply_window(f32_samples, FFT_WINDOW.as_slice());
    // Pad with trailing zeros
    f32_samples
        .iter_mut()
        .for_each(|channel| 
            channel
                .extend(std::iter::repeat(0.0).take(channel.capacity() - channel.len())));

    // Check for silence
    let sound = f32_samples
        .iter()
        .flatten()
        .any(|i| *i != Sample::EQUILIBRIUM);
    if !sound {
        return;
    }
    let volume: f32 = f32_samples
    .iter()
    .map(|c| (c.iter()
        .fold(0.0, |acc, e| acc +  e * e) / c.len() as f32)
        .sqrt())
    .sum::<f32>() / f32_samples.len() as f32;

    let peak = f32_samples
    .iter()
    .map(|c| c.iter()
        .fold(0.0,|max, f| if f.abs() > max {f.abs()} else {max})
    )
    .reduce(f32::max).unwrap();

    info!("RMS: {:.3}, Peak: {:.3}", volume, peak);

    mono_samples.clear();
    mono_samples.extend(f32_samples
        .iter()
        .fold(vec![0.0; f32_samples[0].len()],|sum: Vec<f32>, channel: &Vec<f32>|
            sum
                .iter()
                .zip(channel)
                .map(|(s, c)| *s + c)
                .collect::<Vec<f32>>()
        )
    );
    
    match fft_planner.process(mono_samples, fft_output) {
        Ok(()) => (),
        Err(e) => println!("Error: {:?}", e)
    }

    freq_bins.clear();
    freq_bins.extend(
        fft_output
            .iter()
            .map(|e| (e.re * e.re + e.im * e.im).sqrt())
        );

    let weight: f32 = freq_bins
        .iter()
        .enumerate()
        .map(|(k, freq)| k as f32 * freq)
        .sum();

    if weight >= threshold.get_threshold(weight) {
        println!("{}", "■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■■".bright_red());
    } else {
        println!("{}", "---------------".black());
    }

    let index_of_max = freq_bins
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap()
        .0;

    info!("Loudest frequency: {}Hz", index_of_max);
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
    past_samples: VecDeque<f32>,
    buffer_size: usize
}

#[allow(dead_code)]
impl DynamicThreshold {
    pub fn init() -> Self {
        DynamicThreshold { 
            past_samples: VecDeque::with_capacity(20), buffer_size: 20
        }
    }

    pub fn init_buffer(buffer_size: usize) -> Self {
        DynamicThreshold { 
            past_samples: VecDeque::with_capacity(buffer_size), buffer_size: buffer_size
        }
    }

    fn get_threshold(&mut self, value: f32) -> f32 {
        const DELTA: f32 = 0.2;
        const LAMBDA: f32 = 0.15;
        if self.past_samples.len() >= self.buffer_size {
            self.past_samples.pop_front();
            self.past_samples.push_back(value);
        }
        else {
            self.past_samples.push_back(value);
        }

        let max = self.past_samples.iter().fold(f32::MIN, |a, b| f32::max(a, *b));
        let mut normalized: Vec<f32> = self.past_samples.iter()
            .map(|s| s / max)
            .map(|s| s.powi(2))
            .chain(
                std::iter::repeat(0.0)
                    .take(self.buffer_size - 1)
            )
            .collect();
        let size = normalized.len();
        let wndw: Vec<f32>;
        if self.buffer_size == 20 {
            wndw = THRESHOLD_WINDOW.to_vec();
        } else {
            wndw = window(size, WindowType::Hann);
        }
        apply_window_mono(&mut normalized, wndw.as_slice());
        let sum = normalized.iter().sum::<f32>();
        let threshold = (DELTA + LAMBDA * sum) * max;
        threshold
    }
}


#[allow(dead_code)]
pub enum WindowType {
    Hann,
    FlatTop
}

#[allow(unused_variables, non_snake_case)]
fn window(length: usize, window_type: WindowType) -> Vec<f32> {
    match window_type {
        WindowType::Hann => (0..length)
            .map(|n| 0.5 * (1. - f32::cos(2. * PI * n as f32 / length as f32)))
            .collect::<Vec<f32>>(),
        WindowType::FlatTop => {
            // Matlab coefficents
            const A: [f32; 5] = [0.21557895, 0.41663158, 0.277263158, 0.083578947, 0.006947368]; 
            let window = (0..length)
                .map(|n| 
                    A[0] 
                    - A[1] * (2. * PI * n as f32 / length as f32).cos() 
                    + A[2] * (4. * PI * n as f32 / length as f32).cos() 
                    - A[3] * (6. * PI * n as f32 / length as f32).cos() 
                    + A[4] * (8. * PI * n as f32 / length as f32).cos()
                ).collect::<Vec<f32>>();
                window
        }
    }
}

fn apply_window(samples: &mut Vec<Vec<f32>>, window: &[f32]) {
    samples
        .iter_mut()
        .for_each(|channel|
            apply_window_mono(channel, window)
        );
}

fn apply_window_mono(samples: &mut Vec<f32>, window: &[f32]) {
    samples
        .iter_mut()
        .zip(window)
        .for_each(|(x, w)| *x = *x * w);
}