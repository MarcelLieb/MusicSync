use log::info;

use super::Onset;

use super::{
    threshold::{Dynamic, DynamicSettings},
    OnsetDetector,
};

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

pub struct Hfc {
    threshold: ThresholdBank,
    detection_weights: DetectionWeights,
    bin_resolution: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HfcSettings {
    pub detection_weights: DetectionWeights,
    pub threshold_settings: ThresholdBankSettings,
}

impl Hfc {
    pub fn init(sample_rate: usize, fft_size: usize) -> Self {
        let threshold = ThresholdBank::default();
        let detection_weights = DetectionWeights::default();
        let bin_resolution = sample_rate as f32 / fft_size as f32;
        Self {
            threshold,
            detection_weights,
            bin_resolution,
        }
    }

    pub fn with_settings(sample_rate: usize, fft_size: usize, settings: HfcSettings) -> Self {
        let threshold = ThresholdBank::with_settings(settings.threshold_settings);
        let bin_resolution = sample_rate as f32 / fft_size as f32;
        Self {
            threshold,
            detection_weights: settings.detection_weights,
            bin_resolution,
        }
    }

    pub fn detect(&mut self, freq_bins: &[f32], peak: f32, rms: f32) -> Vec<Onset> {
        let sound = freq_bins.iter().any(|&i| i != 0.0);

        if !sound {
            return vec![];
        }

        let DetectionWeights {
            low_end_weight_cutoff,
            high_end_weight_cutoff,
            mids_weight_low_cutoff,
            mids_weight_high_cutoff,
            drum_click_weight,
            note_click_weight,
        } = self.detection_weights;

        let low_end_weight_cutoff = (low_end_weight_cutoff as f32 / self.bin_resolution) as usize;
        let high_end_weight_cutoff = (high_end_weight_cutoff as f32 / self.bin_resolution) as usize;
        let mids_weight_low_cutoff = (mids_weight_low_cutoff as f32 / self.bin_resolution) as usize;
        let mids_weight_high_cutoff =
            (mids_weight_high_cutoff as f32 / self.bin_resolution) as usize;

        let weight: f32 = freq_bins
            .iter()
            .enumerate()
            .map(|(k, freq)| k as f32 * self.bin_resolution * freq)
            .sum();

        let low_end_weight: &f32 = &freq_bins[0..low_end_weight_cutoff]
            .iter()
            .enumerate()
            .map(|(k, freq)| (k as f32 * self.bin_resolution * *freq))
            .sum::<f32>();

        let high_end_weight: &f32 = &freq_bins[high_end_weight_cutoff..]
            .iter()
            .enumerate()
            .map(|(k, freq)| (k as f32 * self.bin_resolution * *freq))
            .sum::<f32>();

        let mids_weight: &f32 = &freq_bins[mids_weight_low_cutoff..mids_weight_high_cutoff]
            .iter()
            .enumerate()
            .map(|(k, freq)| (k as f32 * self.bin_resolution * *freq))
            .sum::<f32>();

        let index_of_max_mid = (freq_bins[mids_weight_low_cutoff..mids_weight_high_cutoff]
            .iter()
            .enumerate()
            .max_by(|(_, &a), (_, &b)| a.total_cmp(&b))
            .unwrap()
            .0 as f32
            * self.bin_resolution) as usize;

        let index_of_max = (freq_bins
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0 as f32
            * self.bin_resolution) as usize;

        info!("Loudest frequency: {}Hz", index_of_max);

        let mut onsets: Vec<Onset> = Vec::new();

        if weight >= self.threshold.fullband.get_threshold(weight) {
            onsets.push(Onset::Full(rms));
        } else {
            onsets.push(Onset::Atmosphere(rms, index_of_max as u16));
        }

        onsets.push(Onset::Raw(weight));

        let drums_weight = low_end_weight * drum_click_weight * high_end_weight;
        if drums_weight >= self.threshold.drums.get_threshold(drums_weight) {
            onsets.push(Onset::Drum(rms));
        }

        let notes_weight = mids_weight + note_click_weight * high_end_weight;
        if notes_weight >= self.threshold.notes.get_threshold(notes_weight) {
            onsets.push(Onset::Note(rms, index_of_max_mid as u16));
        }

        if *high_end_weight >= self.threshold.hihat.get_threshold(*high_end_weight) {
            onsets.push(Onset::Hihat(peak));
        }
        onsets
    }
}

impl OnsetDetector for Hfc {
    fn detect(&mut self, freq_bins: &[f32], peak: f32, rms: f32) -> Vec<Onset> {
        self.detect(freq_bins, peak, rms)
    }
}

pub struct ThresholdBank {
    pub drums: Dynamic,
    pub hihat: Dynamic,
    pub notes: Dynamic,
    pub fullband: Dynamic,
}

impl Default for ThresholdBank {
    fn default() -> Self {
        let settings = ThresholdBankSettings::default();
        Self {
            drums: Dynamic::with_settings(settings.drums),
            hihat: Dynamic::with_settings(settings.hihat),
            notes: Dynamic::with_settings(settings.notes),
            fullband: Dynamic::with_settings(settings.fullband),
        }
    }
}

impl ThresholdBank {
    pub fn with_settings(settings: ThresholdBankSettings) -> ThresholdBank {
        Self {
            drums: Dynamic::with_settings(settings.drums),
            hihat: Dynamic::with_settings(settings.hihat),
            notes: Dynamic::with_settings(settings.notes),
            fullband: Dynamic::with_settings(settings.fullband),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThresholdBankSettings {
    pub drums: DynamicSettings,
    pub hihat: DynamicSettings,
    pub notes: DynamicSettings,
    pub fullband: DynamicSettings,
}

impl Default for ThresholdBankSettings {
    fn default() -> Self {
        Self {
            drums: DynamicSettings {
                buffer_size: 30,
                min_intensity: 0.3,
                delta_intensity: 0.18,
                ..Default::default()
            },
            hihat: DynamicSettings {
                buffer_size: 20,
                min_intensity: 0.3,
                delta_intensity: 0.18,
                ..Default::default()
            },
            notes: DynamicSettings {
                buffer_size: 20,
                min_intensity: 0.2,
                delta_intensity: 0.15,
                ..Default::default()
            },
            fullband: DynamicSettings {
                buffer_size: 20,
                min_intensity: 0.2,
                delta_intensity: 0.15,
                ..Default::default()
            },
        }
    }
}
