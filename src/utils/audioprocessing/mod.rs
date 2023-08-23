pub mod threshold;
pub mod hfc;

use std::{f32::consts::PI, sync::Arc};

use cpal::Sample;
use dasp_sample::ToSample;
use lazy_static::lazy_static;
use log::info;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

use crate::utils::audiodevices::BUFFER_SIZE;

use self::{threshold::ThresholdSettings, hfc::DetectionWeights};

lazy_static! {
    static ref FFT_WINDOW: Vec<f32> = window(BUFFER_SIZE as usize, WindowType::Hann);
    static ref THRESHOLD_WINDOW: Vec<f32> = window(39, WindowType::Hann);
}

#[derive(Debug)]
pub struct DetectionSettings {
    pub hop_size: usize,
    pub buffer_size: usize,
    pub threshold_settings: ThresholdSettings,
    pub detection_weights: DetectionWeights,
}

impl Default for DetectionSettings {
    fn default() -> DetectionSettings {
        DetectionSettings {
            hop_size: 480,
            buffer_size: 1024,
            threshold_settings: ThresholdSettings::default(),
            detection_weights: DetectionWeights::default(),
        }
    }
}

pub fn prepare_buffers(channels: u16, sample_rate: u32) -> Buffer {
    let mut f32_samples: Vec<Vec<f32>> = Vec::with_capacity(channels.into());
    for _ in 0..channels {
        f32_samples.push(Vec::with_capacity(sample_rate as usize));
    }
    let mono_samples: Vec<f32> = Vec::with_capacity(sample_rate as usize);

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(sample_rate as usize);
    let fft_output: Vec<Vec<Complex<f32>>> = (0..channels)
        .map(|_| fft_planner.make_output_vec())
        .collect();
    let freq_bins: Vec<f32> = vec![0.0; fft_output[0].capacity()];

    return Buffer {
        f32_samples,
        mono_samples,
        fft_output,
        freq_bins,
    };
}

pub struct Buffer {
    f32_samples: Vec<Vec<f32>>,
    mono_samples: Vec<f32>,
    fft_output: Vec<Vec<Complex<f32>>>,
    pub freq_bins: Vec<f32>,
}

pub fn process_raw<T>(
    data: &[T],
    channels: u16,
    fft_planner: &Arc<dyn RealToComplex<f32>>,
    buffer: &mut Buffer,
) -> (f32, f32) where
    T: Sample + ToSample<f32>,
{
    let Buffer {
        f32_samples,
        mono_samples,
        fft_output,
        freq_bins,
    } = buffer;
    
    //Check for silence and abort if present
    let sound = data.iter().any(|i| *i != Sample::EQUILIBRIUM);
    if !sound {
        for channel in f32_samples.iter_mut() {
            channel.clear();
            channel.extend(std::iter::repeat(0.0).take(channel.capacity()));
        }

        mono_samples.clear();
        mono_samples.extend(std::iter::repeat(0.0).take(mono_samples.capacity()));

        freq_bins.clear();
        freq_bins.extend(std::iter::repeat(0.0).take(freq_bins.capacity()));
        return (0.0, 0.0);
    }

    split_channels(channels, data, f32_samples);

    collapse_mono(mono_samples, data, channels);

    let rms: f32 = f32_samples
        .iter()
        .map(|c| (c.iter().fold(0.0, |acc, e| acc + e * e) / c.len() as f32).sqrt())
        .sum::<f32>()
        / channels as f32;

    let peak = f32_samples
        .iter()
        .map(|c| {
            c.iter()
                .fold(0.0, |max, f| if f.abs() > max { f.abs() } else { max })
        })
        .reduce(f32::max)
        .unwrap();

    info!("RMS: {:.3}, Peak: {:.3}", rms, peak);

    fft(f32_samples, fft_output, fft_planner, freq_bins);
    return (peak, rms)
}

fn fft(
    f32_samples: &mut Vec<Vec<f32>>, 
    fft_output: &mut Vec<Vec<Complex<f32>>>, 
    fft_planner: &Arc<dyn RealToComplex<f32>>, 
    freq_bins: &mut Vec<f32>
)
{
    // Could only apply window to collapsed mono signal
    apply_window(f32_samples, FFT_WINDOW.as_slice());
    f32_samples
        .iter_mut()
        .for_each(|chan| chan.extend(std::iter::repeat(0.0).take(chan.capacity() - chan.len())));

    // Calculate FFT
    f32_samples.iter_mut().zip(fft_output.iter_mut()).for_each(
        |(samples, output)| match fft_planner.process(samples, output) {
            Ok(()) => (),
            Err(e) => println!("Error: {:?}", e),
        },
    );
    // Save per channel freq in f32_samples as it has been scrambled already by fft
    fft_output.iter().enumerate().for_each(|(i, out)| {
        f32_samples[i].clear();
        f32_samples[i].extend(out.iter().map(|s| (s.re * s.re + s.im * s.im).sqrt()))
    });

    freq_bins.clear();
    freq_bins.extend((0..fft_output[0].len()).map(|i| {
        f32_samples
            .iter()
            .flatten()
            .skip(i)
            .step_by(f32_samples[0].len())
            .sum::<f32>()
    }));
}

fn collapse_mono<T: Sample + ToSample<f32>>(mono_samples: &mut Vec<f32>, data: &[T], channels: u16) {
    mono_samples.clear();
    // buffer_len != BUFFER_SIZE on linux
    let buffer_len = data.len() / channels as usize;
    // Convert to mono
    mono_samples.extend(
        data.chunks(channels as usize)
            .take(buffer_len)
            .map(|x| x.iter().map(|s| (*s).to_sample::<f32>()).sum::<f32>()),
    );
    // Pad with trailing zeros
    mono_samples.extend(std::iter::repeat(0.0).take(mono_samples.capacity() - mono_samples.len()));
}

fn split_channels<T>(channels: u16, data: &[T], f32_samples: &mut Vec<Vec<f32>>)
where
    T: Sample + ToSample<f32>,
{
    for (i, channel) in f32_samples.iter_mut().enumerate() {
        channel.clear();
        channel.extend(
            data.iter()
                .map(|s| s.to_sample::<f32>())
                .enumerate()
                .filter_map(|(index, f)| {
                    if index % channels as usize == i {
                        Some(f)
                    } else {
                        None
                    }
                }),
        );
    }
}

#[allow(dead_code)]
pub enum WindowType {
    Hann,
    FlatTop,
}

#[allow(unused_variables, non_snake_case)]
fn window(length: usize, window_type: WindowType) -> Vec<f32> {
    match window_type {
        WindowType::Hann => (0..length)
            .map(|n| 0.5 * (1. - f32::cos(2. * PI * n as f32 / length as f32)))
            .collect::<Vec<f32>>(),
        WindowType::FlatTop => {
            // Matlab coefficients
            const A: [f32; 5] = [
                0.21557895,
                0.41663158,
                0.277263158,
                0.083578947,
                0.006947368,
            ];
            let window = (0..length)
                .map(|n| {
                    A[0] - A[1] * (2. * PI * n as f32 / length as f32).cos()
                        + A[2] * (4. * PI * n as f32 / length as f32).cos()
                        - A[3] * (6. * PI * n as f32 / length as f32).cos()
                        + A[4] * (8. * PI * n as f32 / length as f32).cos()
                })
                .collect::<Vec<f32>>();
            window
        }
    }
}

fn apply_window(samples: &mut Vec<Vec<f32>>, window: &[f32]) {
    samples
        .iter_mut()
        .for_each(|channel| apply_window_mono(channel, window));
}

fn apply_window_mono(samples: &mut Vec<f32>, window: &[f32]) {
    samples
        .iter_mut()
        .zip(window)
        .for_each(|(x, w)| *x = *x * w);
}
