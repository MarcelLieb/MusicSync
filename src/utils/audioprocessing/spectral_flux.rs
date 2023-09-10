use crate::utils::lights::{Event, LightService};

use super::{
    threshold::{Advanced, AdvancedSettings},
    MelFilterBank, OnsetDetector,
};

static SNARE: &[f32] = &[
    0.18287805,
    0.24557132,
    0.48058382,
    1.0,
    0.6769353,
    0.46947932,
    0.27236053,
    0.23395863,
    0.32424387,
    0.1672328,
    0.29481238,
    0.1375345,
    0.2470875,
    0.17782561,
    0.24346015,
    0.17866938,
    0.23981878,
    0.23425502,
    0.3624355,
    0.2202465,
    0.18783833,
    0.112611435,
    0.09702215,
    0.12782966,
    0.17580134,
    0.10365773,
    0.09110476,
    0.08804041,
    0.06674518,
    0.05276848,
    0.055121437,
    0.0703828,
    0.04993526,
    0.032884113,
    0.023724684,
    0.01561745,
    0.0114166085,
    0.013744948,
    0.01578424,
    0.027515683,
    0.026058609,
    0.015677225,
    0.009673543,
    0.007576239,
    0.005534774,
    0.008531902,
    0.011647797,
    0.0059992713,
    0.0031900732,
    0.002735502,
    0.003073968,
    0.002507612,
    0.0028221635,
    0.00529891,
    0.0027960262,
    0.0022391793,
    0.0017338615,
    0.001168564,
    0.0006401933,
    0.0005938648,
    0.0011412492,
    0.00097381644,
    0.0006690377,
    0.00038436506,
    0.00027702563,
    0.00024928837,
    0.00025602645,
    0.00016610751,
    0.00018604337,
    0.00012845772,
    9.721349e-05,
    0.00012458027,
    0.0001259657,
    9.898997e-05,
    5.3653566e-05,
    2.8800254e-05,
    2.039804e-05,
    2.1280704e-05,
    2.1281907e-05,
    2.1688396e-05,
    9.452924e-06,
    6.79311e-06,
];

static KICK: &[f32] = &[
    0.54973674,
    1.0,
    0.66868293,
    0.51249576,
    0.24812494,
    0.14619437,
    0.088904984,
    0.056511585,
    0.06650458,
    0.045903906,
    0.06501523,
    0.043217245,
    0.05708676,
    0.038825825,
    0.050472185,
    0.030186648,
    0.040110923,
    0.023096617,
    0.029462136,
    0.017133135,
    0.019188931,
    0.010572069,
    0.009927431,
    0.010494289,
    0.013684456,
    0.0075260866,
    0.00613543,
    0.0053508114,
    0.0047841473,
    0.0032973767,
    0.0029020717,
    0.0041730483,
    0.0030175922,
    0.0017206125,
    0.0011873913,
    0.0006334785,
    0.0006518282,
    0.000691199,
    0.00051863043,
    0.0010091617,
    0.0010551173,
    0.0005567916,
    0.00045593196,
    0.00029509238,
    0.00013585691,
    0.00019859799,
    0.00037875274,
    0.00022537488,
    0.00012296178,
    9.201606e-05,
    0.000118527285,
    8.3154e-05,
    9.544921e-05,
    0.00015113979,
    7.9944555e-05,
    6.167219e-05,
    3.6252237e-05,
    2.0039042e-05,
    1.0842276e-05,
    1.5040062e-05,
    2.787416e-05,
    2.9438821e-05,
    1.6331707e-05,
    8.322525e-06,
    6.2230797e-06,
    4.2950164e-06,
    7.6163406e-06,
    5.629231e-06,
    2.7644105e-06,
    2.6114399e-06,
    3.6960114e-06,
    2.441683e-06,
    3.087708e-06,
    2.3802984e-06,
    1.0907414e-06,
    5.903075e-07,
    5.9604696e-07,
    7.7540386e-07,
    7.6775706e-07,
    6.648186e-07,
    3.0149974e-07,
    2.5325102e-07,
];

static HIHAT: &[f32] = &[
    0.20349485,
    0.13905449,
    0.11798791,
    0.18317275,
    0.18942435,
    0.3516393,
    0.24369457,
    0.3381851,
    0.44916037,
    0.47786075,
    0.6744614,
    0.59866834,
    1.0,
    0.74814415,
    0.856674,
    0.56683,
    0.6533766,
    0.5480646,
    0.8009764,
    0.5628384,
    0.7693119,
    0.43187076,
    0.35033292,
    0.44398978,
    0.49323186,
    0.28828514,
    0.32715368,
    0.31395328,
    0.22536533,
    0.19176558,
    0.18480389,
    0.22174487,
    0.15615016,
    0.09585538,
    0.07861222,
    0.06719542,
    0.069785304,
    0.07364764,
    0.08634601,
    0.14233148,
    0.13584018,
    0.083193,
    0.07277946,
    0.05842322,
    0.03726068,
    0.04824114,
    0.0657125,
    0.043356467,
    0.037424453,
    0.028293798,
    0.022867834,
    0.018867895,
    0.028833054,
    0.045286145,
    0.023664985,
    0.019500405,
    0.01375477,
    0.007889534,
    0.005501038,
    0.0059069544,
    0.009564055,
    0.011232868,
    0.0069247205,
    0.0035839537,
    0.0027410001,
    0.0027802994,
    0.0037219722,
    0.0028698177,
    0.0022971402,
    0.0015649962,
    0.0020539984,
    0.0020026688,
    0.0023319228,
    0.0016016086,
    0.0008908794,
    0.0006692317,
    0.00049506366,
    0.0004962337,
    0.00046560453,
    0.0005216963,
    0.00025297012,
    0.00016382041,
];

pub struct SpecFlux {
    filter_bank: MelFilterBank,
    old_spectrum: Vec<f32>,
    spectrum: Vec<f32>,
    threshold: ThresholdBank,
}

struct ThresholdBank {
    drum: Advanced,
    hihat: Advanced,
    note: Advanced,
    full: Advanced,
}

impl Default for ThresholdBank {
    fn default() -> Self {
        let drum = Advanced::with_settings(AdvancedSettings {
            fixed_threshold: 2.0,
            adaptive_threshold: 0.25,
            mean_range: 5,
            ..Default::default()
        });

        let hihat = Advanced::with_settings(AdvancedSettings {
            fixed_threshold: 6.0,
            adaptive_threshold: 0.6,
            ..Default::default()
        });

        let note = Advanced::with_settings(AdvancedSettings {
            fixed_threshold: 2.0,
            adaptive_threshold: 0.4,
            ..Default::default()
        });

        Self {
            drum,
            hihat,
            note,
            full: Advanced::default(),
        }
    }
}

impl SpecFlux {
    pub fn init(sample_rate: u32, fft_size: u32) -> SpecFlux {
        let bank = MelFilterBank::init(sample_rate, fft_size, 82, 20_000);
        let threshold = ThresholdBank::default();
        let spectrum = vec![0.0; 82];
        let old_spectrum = vec![0.0; 82];
        SpecFlux {
            filter_bank: bank,
            spectrum,
            old_spectrum,
            threshold,
        }
    }

    pub fn detect(
        &mut self,
        freq_bins: &[f32],
        peak: f32,
        rms: f32,
        lightservices: &mut [Box<dyn LightService + Send>],
    ) {
        self.old_spectrum.clear();
        self.old_spectrum.extend(&self.spectrum);

        let lambda = 0.1;

        self.filter_bank.filter(freq_bins, &mut self.spectrum);

        self.spectrum
            .iter_mut()
            .for_each(|x| *x = (*x * lambda).ln_1p());

        let diff = self
            .old_spectrum
            .iter()
            .zip(&self.spectrum)
            .map(|(&a, &b)| (((b - a) + (b - a).abs()) / 2.0));

        let weight: f32 = diff.clone().sum();

        let drum_weight: f32 = diff.clone().zip(KICK).map(|(d, &w)| d * w).sum();

        let hihat_weight: f32 = diff.clone().zip(HIHAT).map(|(d, &w)| d * w).sum();

        let note_weight: f32 = diff.clone().zip(SNARE).map(|(d, &w)| d * w).sum();

        let onset = self.threshold.full.is_above(weight);

        let index_of_max = freq_bins
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        lightservices.event_detected(Event::Raw(drum_weight));

        if onset {
            lightservices.event_detected(Event::Full(rms));
        }

        if self.threshold.drum.is_above(drum_weight) {
            lightservices.event_detected(Event::Drum(rms));
        }

        if self.threshold.hihat.is_above(hihat_weight) {
            lightservices.event_detected(Event::Hihat(peak));
        }

        if self.threshold.note.is_above(note_weight) {
            lightservices.event_detected(Event::Note(rms, index_of_max as u16));
        }

        lightservices.update();
    }
}

impl OnsetDetector for SpecFlux {
    fn detect(
        &mut self,
        freq_bins: &[f32],
        peak: f32,
        rms: f32,
        lightservices: &mut [Box<dyn LightService + Send>],
    ) {
        self.detect(freq_bins, peak, rms, lightservices);
    }
}
