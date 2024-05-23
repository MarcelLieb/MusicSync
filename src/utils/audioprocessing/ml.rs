use ndarray::{s, Array1, Array2, Axis};
use ort::{inputs, Session};

use crate::utils::audioprocessing::Onset;

use super::{threshold, MelFilterBank, OnsetDetector};

pub struct ThresholdBank {
    pub kick: threshold::Advanced,
    pub snare: threshold::Advanced,
    pub hihat: threshold::Advanced,
}

pub struct ThresholdBankSettings {
    pub kick: threshold::AdvancedSettings,
    pub snare: threshold::AdvancedSettings,
    pub hihat: threshold::AdvancedSettings,
}

impl Default for ThresholdBankSettings {
    fn default() -> Self {
        Self {
            kick: threshold::AdvancedSettings {
                mean_range: 2,
                max_range: 2,
                dynamic_threshold: 0.0,
                threshold_range: 2,
                fixed_threshold: 0.1,
                delay: 0,
            },
            snare: threshold::AdvancedSettings {
                mean_range: 2,
                max_range: 2,
                dynamic_threshold: 0.0,
                threshold_range: 2,
                fixed_threshold: 0.1,
                delay: 0,
            },
            hihat: threshold::AdvancedSettings {
                mean_range: 2,
                max_range: 2,
                dynamic_threshold: 0.0,
                threshold_range: 2,
                fixed_threshold: 0.1,
                delay: 0,
            },
        }
    }
}

impl ThresholdBank {
    pub fn with_settings(settings: ThresholdBankSettings) -> Self {
        Self {
            kick: threshold::Advanced::with_settings(settings.kick),
            snare: threshold::Advanced::with_settings(settings.snare),
            hihat: threshold::Advanced::with_settings(settings.hihat),
        }
    }
}

impl Default for ThresholdBank {
    fn default() -> Self {
        Self::with_settings(ThresholdBankSettings::default())
    }
}

pub struct MLDetector {
    filter_bank: MelFilterBank,
    session: Session,
    threshold: ThresholdBank,
    buffer: Array2<f32>,
    vec_buffer: Vec<f32>,
}

impl MLDetector {
    pub fn init(sample_rate: u32, fft_size: u32) -> ort::Result<Self> {
        let filter_bank = MelFilterBank::init(sample_rate, fft_size, 84, 20_000);
        let session = Session::builder()?.commit_from_file("./path/to/model")?;
        let buffer = ndarray::Array2::zeros((84, 15));
        let threshold = ThresholdBank::default();
        Ok(Self {
            filter_bank,
            session,
            buffer,
            threshold,
            vec_buffer: Vec::with_capacity(84),
        })
    }
}

impl OnsetDetector for MLDetector {
    fn detect(&mut self, freq_bins: &[f32], peak: f32, rms: f32) -> Vec<super::Onset> {
        self.filter_bank.filter(freq_bins, &mut self.vec_buffer);
        let array = Array1::from_vec(self.vec_buffer.clone());
        self.buffer
            .append(Axis(0), array.view().insert_axis(Axis(0)))
            .unwrap();
        self.buffer.remove_index(Axis(0), 0);
        // TODO: Log errors
        let inputs = inputs![self.buffer.view()].unwrap();
        let outputs = self.session.run(inputs).unwrap();
        let output = outputs["output0"]
            .try_extract_tensor::<f32>()
            .unwrap()
            .into_owned();
        let output: Vec<_> = output.slice(s![.., -1]).iter().cloned().collect();
        let mut onsets = Vec::new();

        if self.threshold.kick.is_above(output[0]) {
            onsets.push(Onset::Kick(rms));
        }

        if self.threshold.snare.is_above(output[1]) {
            onsets.push(Onset::Snare(rms));
        }

        if self.threshold.hihat.is_above(output[2]) {
            onsets.push(Onset::Hihat(peak * output[2]))
        }

        if !onsets.is_empty() {
            onsets.push(Onset::Full(rms))
        }

        onsets
    }
}
