use std::{collections::VecDeque, f32::consts::PI, sync::Arc};

use cpal::Sample;
use dasp_sample::ToSample;
use lazy_static::lazy_static;
use log::info;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;

use crate::utils::{
    audiodevices::BUFFER_SIZE,
    lights::{Event, LightService},
};

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
            hop_size: 360,
            buffer_size: 480,
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
    freq_bins: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct DetectionWeights {
    pub low_end_weight_cutoff: usize,
    pub high_end_weight_cutoff: usize,
    pub mids_weight_low_cutoff: usize,
    pub mids_weight_high_cutoff: usize,
    pub drum_click_weight: f32,
    pub note_click_weight: f32,
}

impl Default for DetectionWeights {
    fn default() -> DetectionWeights {
        DetectionWeights {
            low_end_weight_cutoff: 300,
            high_end_weight_cutoff: 2000,
            mids_weight_low_cutoff: 200,
            mids_weight_high_cutoff: 3000,
            drum_click_weight: 0.005,
            note_click_weight: 0.1,
        }
    }
}

pub fn print_onset<T>(
    data: &[T],
    channels: u16,
    fft_planner: &Arc<dyn RealToComplex<f32>>,
    buffer: &mut Buffer,
    threshold: &mut MultiBandThreshold,
    lightservices: &mut [Box<dyn LightService + Send>],
    detection_weights: Option<&DetectionWeights>,
) where
    T: Sample + ToSample<f32>,
{
    let Buffer {
        f32_samples,
        mono_samples,
        fft_output,
        freq_bins,
    } = buffer;

    let detection_weights = *(detection_weights.unwrap_or(&DetectionWeights::default()));

    //Check for silence and abort if present
    let sound = data.iter().any(|i| *i != Sample::EQUILIBRIUM);
    if !sound {
        lightservices
            .iter_mut()
            .for_each(|service| service.update());
        return;
    }

    split_channels(channels, data, f32_samples);

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

    collapse_mono(mono_samples, data, channels);

    hfc(freq_bins, threshold, detection_weights, lightservices, rms, peak);
    
    lightservices
        .iter_mut()
        .for_each(|service| service.update());
}

fn hfc(freq_bins: &mut Vec<f32>, threshold: &mut MultiBandThreshold, detection_weights: DetectionWeights, lightservices: &mut [Box<dyn LightService + Send>], rms: f32, peak: f32) {
    let DetectionWeights {
        low_end_weight_cutoff,
        high_end_weight_cutoff,
        mids_weight_low_cutoff,
        mids_weight_high_cutoff,
        drum_click_weight,
        note_click_weight,
    } = detection_weights;
    
    let weight: f32 = freq_bins
        .iter()
        .enumerate()
        .map(|(k, freq)| k as f32 * freq)
        .sum();

    let low_end_weight: &f32 = &freq_bins[50..low_end_weight_cutoff]
        .iter()
        .enumerate()
        .map(|(k, freq)| (k as f32 * *freq))
        .sum::<f32>();

    let high_end_weight: &f32 = &freq_bins[high_end_weight_cutoff..]
        .iter()
        .enumerate()
        .map(|(k, freq)| (k as f32 * *freq))
        .sum::<f32>();

    let mids_weight: &f32 = &freq_bins[mids_weight_low_cutoff..mids_weight_high_cutoff]
        .iter()
        .enumerate()
        .map(|(k, freq)| (k as f32 * *freq))
        .sum::<f32>();

    let index_of_max_mid = &freq_bins[mids_weight_low_cutoff..mids_weight_high_cutoff]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap()
        .0;

    let index_of_max = freq_bins
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .unwrap()
        .0;

    info!("Loudest frequency: {}Hz", index_of_max);

    if weight >= threshold.fullband.get_threshold(weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Full(rms)));
    } else {
        lightservices.iter_mut().for_each(|service| {
            service.event_detected(Event::Atmosphere(rms, index_of_max as u16))
        });
    }

    let drums_weight = low_end_weight * drum_click_weight * high_end_weight;
    if drums_weight >= threshold.drums.get_threshold(drums_weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Drum(rms)));
    }

    let notes_weight = mids_weight + note_click_weight * high_end_weight;
    if notes_weight >= threshold.notes.get_threshold(notes_weight) {
        lightservices.iter_mut().for_each(|service| {
            service.event_detected(Event::Note(rms, *index_of_max_mid as u16))
        });
    }

    if *high_end_weight >= threshold.hihat.get_threshold(*high_end_weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Hihat(peak)));
    }
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

#[derive(Debug, Clone)]
pub struct DynamicThreshold {
    past_samples: VecDeque<f32>,
    buffer_size: usize,
    min_intensity: f32,
    delta_intensity: f32,
}

#[allow(dead_code)]
impl DynamicThreshold {
    pub fn init() -> Self {
        DynamicThreshold {
            past_samples: VecDeque::with_capacity(20),
            buffer_size: 20,
            min_intensity: 0.2,
            delta_intensity: 0.15,
        }
    }

    pub fn init_config(
        buffer_size: usize,
        min_intensity: Option<f32>,
        delta_intensity: Option<f32>,
    ) -> Self {
        DynamicThreshold {
            past_samples: VecDeque::with_capacity(buffer_size),
            buffer_size: buffer_size,
            min_intensity: min_intensity.unwrap_or(0.2),
            delta_intensity: delta_intensity.unwrap_or(0.15),
        }
    }

    fn get_threshold(&mut self, value: f32) -> f32 {
        if self.past_samples.len() >= self.buffer_size {
            self.past_samples.pop_front();
            self.past_samples.push_back(value);
        } else {
            self.past_samples.push_back(value);
        }

        let max = self
            .past_samples
            .iter()
            .fold(f32::MIN, |a, b| f32::max(a, *b));
        let mut normalized: Vec<f32> = self
            .past_samples
            .iter()
            .map(|s| s / max)
            .map(|s| s.powi(2))
            .chain(std::iter::repeat(0.0).take(self.buffer_size - 1))
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
        let threshold = (self.min_intensity + self.delta_intensity * sum) * max;
        threshold
    }
}

impl Default for DynamicThreshold {
    fn default() -> Self {
        DynamicThreshold::init()
    }
}

pub struct MultiBandThreshold {
    pub drums: DynamicThreshold,
    pub hihat: DynamicThreshold,
    pub notes: DynamicThreshold,
    pub fullband: DynamicThreshold,
}

#[derive(Debug)]
pub struct ThresholdSettings {
    pub drum_buffer: usize,
    pub drum_min_intensity: f32,
    pub drum_delta_intensity: f32,
    pub hihat_buffer: usize,
    pub hihat_min_intensity: f32,
    pub hihat_delta_intensity: f32,
    pub note_buffer: usize,
    pub note_min_intensity: f32,
    pub note_delta_intensity: f32,
    pub full_buffer: usize,
    pub full_min_intensity: f32,
    pub full_delta_intensity: f32,
}

impl Default for ThresholdSettings {
    fn default() -> Self {
        Self {
            drum_buffer: 30,
            drum_min_intensity: 0.3,
            drum_delta_intensity: 0.18,
            hihat_buffer: 20,
            hihat_min_intensity: 0.3,
            hihat_delta_intensity: 0.18,
            note_buffer: 20,
            note_min_intensity: 0.2,
            note_delta_intensity: 0.15,
            full_buffer: 20,
            full_min_intensity: 0.2,
            full_delta_intensity: 0.15,
        }
    }
}

impl Default for MultiBandThreshold {
    fn default() -> Self {
        let settings = ThresholdSettings::default();
        Self {
            drums: DynamicThreshold::init_config(
                settings.drum_buffer,
                Some(settings.drum_min_intensity),
                Some(settings.drum_delta_intensity),
            ),
            hihat: DynamicThreshold::init_config(
                settings.hihat_buffer,
                Some(settings.hihat_min_intensity),
                Some(settings.hihat_delta_intensity),
            ),
            notes: DynamicThreshold::init_config(
                settings.note_buffer,
                Some(settings.note_min_intensity),
                Some(settings.note_delta_intensity),
            ),
            fullband: DynamicThreshold::init_config(
                settings.full_buffer,
                Some(settings.full_min_intensity),
                Some(settings.full_delta_intensity),
            ),
        }
    }
}

impl MultiBandThreshold {
    pub fn init_settings(settings: ThresholdSettings) -> MultiBandThreshold {
        Self {
            drums: DynamicThreshold::init_config(
                settings.drum_buffer,
                Some(settings.drum_min_intensity),
                Some(settings.drum_delta_intensity),
            ),
            hihat: DynamicThreshold::init_config(
                settings.hihat_buffer,
                Some(settings.hihat_min_intensity),
                Some(settings.hihat_delta_intensity),
            ),
            notes: DynamicThreshold::init_config(
                settings.note_buffer,
                Some(settings.note_min_intensity),
                Some(settings.note_delta_intensity),
            ),
            fullband: DynamicThreshold::init_config(
                settings.full_buffer,
                Some(settings.full_min_intensity),
                Some(settings.full_delta_intensity),
            ),
        }
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
            // Matlab coefficents
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
