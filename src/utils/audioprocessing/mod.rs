pub mod hfc;
pub mod spectral_flux;
pub mod threshold;

use std::{f32::consts::PI, sync::Arc};

use cpal::Sample;
use dasp_sample::ToSample;
use log::info;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

use super::lights::LightService;

#[derive(Debug, Clone, Copy)]
pub struct ProcessingSettings {
    pub sample_rate: u32,
    pub hop_size: usize,
    pub buffer_size: usize,
    pub fft_size: usize,
    pub window_type: WindowType,
}

impl Default for ProcessingSettings {
    fn default() -> ProcessingSettings {
        ProcessingSettings {
            sample_rate: 48000,
            hop_size: 480,
            buffer_size: 1024,
            fft_size: 2048,
            window_type: WindowType::Hann,
        }
    }
}

pub fn prepare_buffers(channels: u16, settings: &ProcessingSettings) -> Buffer {
    let mut f32_samples: Vec<Vec<f32>> = Vec::with_capacity(channels.into());
    for _ in 0..channels {
        f32_samples.push(vec![0.0; settings.fft_size]);
    }
    let mono_samples: Vec<f32> = vec![0.0; settings.buffer_size];

    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(settings.fft_size);
    let fft_output: Vec<Vec<Complex<f32>>> = (0..channels)
        .map(|_| fft_planner.make_output_vec())
        .collect();
    let freq_bins: Vec<f32> = vec![0.0; fft_output[0].capacity()];
    let fft_window = window(settings.buffer_size, settings.window_type);

    Buffer {
        f32_samples,
        mono_samples,
        fft_output,
        fft_window,
        freq_bins,
        fft_planner,
        peak: 0.0,
        rms: 0.0,
    }
}

pub struct Buffer {
    f32_samples: Vec<Vec<f32>>,
    pub mono_samples: Vec<f32>,
    fft_output: Vec<Vec<Complex<f32>>>,
    fft_window: Vec<f32>,
    pub freq_bins: Vec<f32>,
    fft_planner: Arc<dyn RealToComplex<f32>>,
    pub peak: f32,
    pub rms: f32,
}

pub fn process_raw<T>(data: &[T], channels: u16, buffer: &mut Buffer)
where
    T: Sample + ToSample<f32>,
{
    let Buffer {
        f32_samples,
        mono_samples,
        fft_output,
        freq_bins,
        fft_window,
        fft_planner,
        peak,
        rms,
    } = buffer;

    //Check for silence and abort if present
    let sound = data.iter().any(|i| *i != Sample::EQUILIBRIUM);
    if !sound {
        for channel in &mut *f32_samples {
            channel.clear();
            channel.extend(std::iter::repeat(0.0).take(channel.capacity()));
        }

        mono_samples.clear();
        mono_samples.extend(std::iter::repeat(0.0).take(mono_samples.capacity()));

        freq_bins.clear();
        freq_bins.extend(std::iter::repeat(0.0).take(freq_bins.capacity()));
        *peak = 0.0;
        *rms = 0.0;
        return;
    }

    split_channels(channels, data, f32_samples);

    collapse_mono(mono_samples, data, channels);

    *rms = f32_samples
        .iter()
        .map(|c| (c.iter().fold(0.0, |acc, e| acc + e * e) / c.len() as f32).sqrt())
        .sum::<f32>()
        / channels as f32;

    *peak = f32_samples
        .iter()
        .map(|c| {
            c.iter()
                .fold(0.0, |max, f| if f.abs() > max { f.abs() } else { max })
        })
        .reduce(f32::max)
        .unwrap();

    info!("RMS: {:.3}, Peak: {:.3}", rms, peak);

    fft(f32_samples, fft_output, fft_planner, fft_window, freq_bins);
}

fn fft(
    f32_samples: &mut [Vec<f32>],
    fft_output: &mut [Vec<Complex<f32>>],
    fft_planner: &Arc<dyn RealToComplex<f32>>,
    fft_window: &[f32],
    freq_bins: &mut [f32],
) {
    // Could only apply window to collapsed mono signal
    apply_window(f32_samples, fft_window);
    f32_samples
        .iter_mut()
        .for_each(|chan| chan.extend(std::iter::repeat(0.0).take(chan.capacity() - chan.len())));

    // Calculate FFT
    f32_samples.iter_mut().zip(fft_output.iter_mut()).for_each(
        |(samples, output)| match fft_planner.process(samples, output) {
            Ok(()) => (),
            Err(e) => println!("Error: {e:?}"),
        },
    );
    // Save per channel freq in f32_samples as it has been scrambled already by fft
    fft_output.iter().enumerate().for_each(|(i, out)| {
        f32_samples[i].clear();
        f32_samples[i].extend(out.iter().map(|s| (s.re * s.re + s.im * s.im).sqrt()));
    });

    freq_bins
        .iter_mut()
        .zip((0..fft_output[0].len()).map(|i| {
            f32_samples
                .iter()
                .flatten()
                .skip(i)
                .step_by(f32_samples[0].len())
                .sum::<f32>()
        }))
        .for_each(|(f, s)| *f = s);
}

fn collapse_mono<T: Sample + ToSample<f32>>(mono_samples: &mut [f32], data: &[T], channels: u16) {
    let buffer_len = data.len() / channels as usize;
    // Convert to mono
    mono_samples
        .iter_mut()
        .zip(
            data.chunks(channels as usize)
                .take(buffer_len)
                .map(|x| x.iter().map(|s| (*s).to_sample::<f32>()).sum::<f32>()),
        )
        .for_each(|(m, s)| *m = s);
}

fn split_channels<T>(channels: u16, data: &[T], f32_samples: &mut [Vec<f32>])
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
#[derive(Debug, Clone, Copy)]
pub enum WindowType {
    Hann,
    FlatTop,
    Triangular,
}

#[allow(unused_variables, non_snake_case)]
fn window(length: usize, window_type: WindowType) -> Vec<f32> {
    match window_type {
        WindowType::Hann => (0..length)
            .map(|n| 0.5 * (1. - f32::cos(2. * PI * n as f32 / length as f32)))
            .collect::<Vec<f32>>(),
        WindowType::FlatTop => {
            // Matlab coefficients
            const A: [f32; 5] = [0.21557895, 0.41663158, 0.27726316, 0.083578947, 0.006947368];
            (0..length)
                .map(|n| {
                    A[0] - A[1] * (2. * PI * n as f32 / length as f32).cos()
                        + A[2] * (4. * PI * n as f32 / length as f32).cos()
                        - A[3] * (6. * PI * n as f32 / length as f32).cos()
                        + A[4] * (8. * PI * n as f32 / length as f32).cos()
                })
                .collect::<Vec<f32>>()
        }
        WindowType::Triangular => (0..length)
            .map(|n| 1.0 - (2.0 * n as f32 / length as f32 - 1.0).abs())
            .collect::<Vec<f32>>(),
    }
}

fn apply_window(samples: &mut [Vec<f32>], window: &[f32]) {
    samples
        .iter_mut()
        .for_each(|channel| apply_window_mono(channel, window));
}

fn apply_window_mono(samples: &mut [f32], window: &[f32]) {
    samples.iter_mut().zip(window).for_each(|(x, w)| *x *= w);
}

pub struct MelFilterBank {
    filter: Vec<Vec<f32>>,
    points: Vec<f32>,
    pub fft_size: u32,
    pub bands: usize,
    pub sample_rate: u32,
    pub max_frequency: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct MelFilterBankSettings {
    pub bands: usize,
    pub max_frequency: u32,
}

impl Default for MelFilterBankSettings {
    fn default() -> Self {
        Self {
            bands: 82,
            max_frequency: 20_000,
        }
    }
}

impl MelFilterBank {
    pub fn init(
        sample_rate: u32,
        fft_size: u32,
        bands: usize,
        max_frequency: u32,
    ) -> MelFilterBank {
        let num_points = bands + 2;
        let mel_max = Self::hertz_to_mel(max_frequency as f32);
        let step = mel_max / (num_points - 1) as f32;

        let mel = (0..num_points)
            .map(|i| i as f32 * step)
            .map(Self::mel_to_hertz)
            .collect::<Vec<f32>>();

        let bin_res = sample_rate as f32 / fft_size as f32;

        let mut filter: Vec<Vec<f32>> = Vec::new();

        (1..=bands).for_each(|m| {
            let start = (mel[m - 1] / bin_res) as usize;
            let mid = (mel[m] / bin_res) as usize;
            let end = (mel[m + 1] / bin_res) as usize;

            let mut band: Vec<f32> = Vec::new();

            for k in start..mid {
                band.push((k - start) as f32 / (mid - start) as f32);
            }
            for k in mid..end {
                band.push((end - k) as f32 / (end - mid) as f32);
            }

            filter.push(band);
        });

        MelFilterBank {
            filter,
            points: mel,
            fft_size,
            bands,
            sample_rate,
            max_frequency,
        }
    }

    pub fn with_settings(
        sample_rate: u32,
        fft_size: u32,
        settings: MelFilterBankSettings,
    ) -> MelFilterBank {
        MelFilterBank::init(
            sample_rate,
            fft_size,
            settings.bands,
            settings.max_frequency,
        )
    }

    pub fn filter(&self, freq_bins: &[f32], out: &mut [f32]) {
        let bin_res = self.sample_rate as f32 / self.fft_size as f32;

        self.filter
            .iter()
            .zip(out)
            .enumerate()
            .for_each(|(m, (band, x))| {
                let start = (self.points[m] / bin_res) as usize;
                let sum = freq_bins
                    .iter()
                    .skip(start)
                    .take(band.len())
                    .zip(band)
                    .map(|(&f, &w)| f * w)
                    .sum::<f32>();

                *x = sum;
            });
    }

    pub fn hertz_to_mel(hertz: f32) -> f32 {
        1127.0 * (hertz / 700.0).ln_1p()
    }

    pub fn mel_to_hertz(mel: f32) -> f32 {
        700.0 * (mel / 1127.0).exp_m1()
    }
}

trait OnsetDetector {
    fn detect(
        &mut self,
        freq_bins: &[f32],
        peak: f32,
        rms: f32,
        lightservices: &mut [Box<dyn LightService + Send>],
    );
}
