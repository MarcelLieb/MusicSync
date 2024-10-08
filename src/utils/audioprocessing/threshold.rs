use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use super::{apply_window_mono, window, WindowType};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, PartialOrd)]
#[serde(default)]
pub struct DynamicSettings {
    pub buffer_size: usize,
    pub min_intensity: f32,
    pub delta_intensity: f32,
    pub window_type: WindowType,
}

impl Default for DynamicSettings {
    fn default() -> Self {
        Self {
            buffer_size: 20,
            min_intensity: 0.2,
            delta_intensity: 0.15,
            window_type: WindowType::Hann,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dynamic {
    past_samples: VecDeque<f32>,
    buffer_size: usize,
    min_intensity: f32,
    delta_intensity: f32,
    window: Vec<f32>,
}

#[allow(dead_code)]
impl Dynamic {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: DynamicSettings) -> Self {
        let DynamicSettings {
            buffer_size,
            min_intensity,
            delta_intensity,
            window_type,
        } = settings;
        Dynamic {
            past_samples: VecDeque::with_capacity(buffer_size),
            buffer_size,
            min_intensity,
            delta_intensity,
            window: window(buffer_size, window_type),
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
            .chain(std::iter::repeat(0.0).take(self.buffer_size - self.past_samples.len()))
            .collect();

        apply_window_mono(&mut normalized, &self.window);

        let sum = normalized.iter().sum::<f32>();
        (self.min_intensity + self.delta_intensity * sum) * max
    }

    pub fn is_above(&mut self, value: f32) -> bool {
        value > self.get_threshold(value)
    }
}

impl Default for Dynamic {
    fn default() -> Self {
        Dynamic::with_settings(DynamicSettings::default())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, PartialOrd)]
#[serde(default)]
pub struct AdvancedSettings {
    pub mean_range: usize,
    pub max_range: usize,
    pub dynamic_threshold: f32,
    pub threshold_range: usize,
    pub fixed_threshold: f32,
    pub delay: usize,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        AdvancedSettings {
            mean_range: 6,
            max_range: 3,
            dynamic_threshold: 0.8,
            threshold_range: 8,
            fixed_threshold: 0.5,
            delay: 2,
        }
    }
}

pub struct Advanced {
    past_samples: VecDeque<f32>,
    mean_range: usize,
    max_range: usize,
    dynamic_threshold: f32,
    threshold_range: usize,
    fixed_threshold: f32,
    delay: usize,
    delay_slots: VecDeque<bool>,
}

impl Advanced {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn with_settings(settings: AdvancedSettings) -> Self {
        let len = settings
            .max_range
            .max(settings.mean_range)
            .max(settings.threshold_range);
        Advanced {
            past_samples: VecDeque::from(vec![0.0; len]),
            mean_range: settings.mean_range,
            max_range: settings.max_range,
            dynamic_threshold: settings.dynamic_threshold,
            threshold_range: settings.threshold_range,
            fixed_threshold: settings.fixed_threshold,
            delay: settings.delay,
            delay_slots: VecDeque::from(vec![false; settings.delay + 1]),
        }
    }

    pub fn is_above(&mut self, value: f32) -> bool {
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

        self.past_samples.pop_front();
        self.past_samples.push_back(value);

        let onset = value >= max
            && value >= mean + norm * self.dynamic_threshold + self.fixed_threshold
            && !self.delay_slots[0];
        self.delay_slots.pop_back();
        self.delay_slots.push_front(onset);

        self.delay_slots[self.delay]
    }
}

impl Default for Advanced {
    fn default() -> Self {
        Advanced::with_settings(AdvancedSettings::default())
    }
}
