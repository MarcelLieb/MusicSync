use log::info;

use crate::utils::lights::{Event, LightService};

use super::threshold::MultiBandThreshold;

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

pub fn hfc(
    freq_bins: &Vec<f32>,
    peak: f32,
    rms: f32,
    threshold: &mut MultiBandThreshold,
    detection_weights: Option<&DetectionWeights>,
    lightservices: &mut [Box<dyn LightService + Send>],
) {
    let sound = freq_bins.iter().any(|&i| i != 0.0);

    if !sound {
        lightservices
            .iter_mut()
            .for_each(|service| service.update());
    }

    let detection_weights = *(detection_weights.unwrap_or(&DetectionWeights::default()));

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

    lightservices
        .iter_mut()
        .for_each(|service| service.event_detected(Event::Raw(weight)));

    let drums_weight = low_end_weight * drum_click_weight * high_end_weight;
    if drums_weight >= threshold.drums.get_threshold(drums_weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Drum(rms)));
    }

    let notes_weight = mids_weight + note_click_weight * high_end_weight;
    if notes_weight >= threshold.notes.get_threshold(notes_weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Note(rms, *index_of_max_mid as u16)));
    }

    if *high_end_weight >= threshold.hihat.get_threshold(*high_end_weight) {
        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Hihat(peak)));
    }

    lightservices
        .iter_mut()
        .for_each(|service| service.update());
}
