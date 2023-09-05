use lazy_static::lazy_static;
use std::collections::VecDeque;

use super::{apply_window_mono, window, WindowType};

lazy_static! {
    static ref THRESHOLD_WINDOW: Vec<f32> = window(39, WindowType::Hann);
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

    pub fn with_settings(settings: DynamicSettings) -> Self {
        let DynamicSettings {
            buffer_size,
            min_intensity,
            delta_intensity,
        } = settings;
        DynamicThreshold {
            past_samples: VecDeque::with_capacity(buffer_size),
            buffer_size,
            min_intensity,
            delta_intensity,
        }
    }

    pub fn get_threshold(&mut self, value: f32) -> f32 {
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
        let wndw: Vec<f32> = if self.buffer_size == 20 {
            THRESHOLD_WINDOW.to_vec()
        } else {
            window(size, WindowType::Hann)
        };
        apply_window_mono(&mut normalized, wndw.as_slice());
        let sum = normalized.iter().sum::<f32>();
        (self.min_intensity + self.delta_intensity * sum) * max
    }
}

pub struct DynamicSettings {
    pub buffer_size: usize,
    pub min_intensity: f32,
    pub delta_intensity: f32,
}

impl Default for DynamicSettings {
    fn default() -> Self {
        Self {
            buffer_size: 20,
            min_intensity: 0.2,
            delta_intensity: 0.15,
        }
    }
}

impl Default for DynamicThreshold {
    fn default() -> Self {
        DynamicThreshold {
            past_samples: VecDeque::with_capacity(20),
            buffer_size: 20,
            min_intensity: 0.2,
            delta_intensity: 0.15,
        }
    }
}

pub struct AdvancedThreshold {
    past_samples: VecDeque<f32>,
    last_onset: u32,
    mean_range: usize,
    max_range: usize,
    dynamic_threshold: f32,
    threshold_range: usize,
    fixed_threshold: f32,
}

impl AdvancedThreshold {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: AdvancedSettings) -> Self {
        let AdvancedSettings {
            mean_range,
            max_range,
            adaptive_threshold,
            threshold_range,
            fixed_threshold,
        } = settings;
        let mut past_samples = VecDeque::with_capacity(12);

        past_samples.extend(vec![0.0; 8]);
        AdvancedThreshold {
            past_samples,
            last_onset: 0,
            mean_range,
            max_range,
            dynamic_threshold: adaptive_threshold,
            threshold_range,
            fixed_threshold,
        }
    }

    pub fn is_above(&mut self, value: f32) -> bool {
        self.last_onset += 1;
        let max = self
            .past_samples
            .iter()
            .take(self.max_range)
            .fold(0.0_f32, |a, &b| a.max(b));
        let mean =
            self.past_samples.iter().take(self.mean_range).sum::<f32>() / self.mean_range as f32;
        let norm = self
            .past_samples
            .iter()
            .take(self.threshold_range)
            .sum::<f32>()
            / self.threshold_range as f32;

        if self.past_samples.len() >= self.past_samples.capacity() {
            self.past_samples.pop_back();
            self.past_samples.push_front(value);
        } else {
            self.past_samples.push_front(value);
        }
        if value >= max && value >= mean + norm * self.dynamic_threshold + self.fixed_threshold {
            self.last_onset = 0;
        }
        self.last_onset == 2
    }
}

pub struct AdvancedSettings {
    pub mean_range: usize,
    pub max_range: usize,
    pub adaptive_threshold: f32,
    pub threshold_range: usize,
    pub fixed_threshold: f32,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        AdvancedSettings {
            mean_range: 6,
            max_range: 3,
            adaptive_threshold: 0.8,
            threshold_range: 8,
            fixed_threshold: 5.0,
        }
    }
}

impl Default for AdvancedThreshold {
    fn default() -> Self {
        let mut past_samples = VecDeque::with_capacity(12);
        past_samples.extend(vec![0.0; 8]);
        AdvancedThreshold {
            past_samples,
            last_onset: 0,
            mean_range: 6,
            max_range: 3,
            dynamic_threshold: 0.8,
            threshold_range: 8,
            fixed_threshold: 5.0,
        }
    }
}
