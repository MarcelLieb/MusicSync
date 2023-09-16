use crate::utils::lights::{Event, LightService};

use super::{
    threshold::{Advanced, AdvancedSettings},
    MelFilterBank, OnsetDetector,
};

static SNARE_MASK: &[f32] = &[
    0.0744277,
    0.1444252,
    0.33383515,
    0.6730734,
    0.9558905,
    1.0,
    0.74690217,
    0.5114114,
    0.37169942,
    0.28074843,
    0.24411103,
    0.1803879,
    0.15899612,
    0.14451577,
    0.14423437,
    0.13805827,
    0.1330982,
    0.14528356,
    0.15683472,
    0.14177422,
    0.11870477,
    0.09817378,
    0.08714519,
    0.087238766,
    0.07459925,
    0.06506758,
    0.055177364,
    0.055802066,
    0.05377244,
    0.04069749,
    0.032626554,
    0.030884603,
    0.026029076,
    0.02591511,
    0.0268765,
    0.021884492,
    0.016387679,
    0.016970856,
    0.017857322,
    0.01492204,
    0.012160584,
    0.010717618,
    0.008181746,
    0.007268617,
    0.008158705,
    0.0063360203,
    0.005507253,
    0.005033175,
    0.0049971803,
    0.004334938,
    0.0029526562,
    0.0026871832,
    0.0029996394,
    0.0024192033,
    0.0018825653,
    0.0014814448,
    0.0014326146,
    0.0011892413,
    0.0009521914,
    0.00075347314,
    0.0006746325,
    0.00073097006,
    0.0005541812,
    0.0004551946,
    0.00037913007,
    0.00042164006,
    0.0003021642,
    0.00024542992,
    0.00021589742,
    0.00020816489,
    0.00014505848,
    0.00012303329,
    9.945266e-05,
    7.5203665e-05,
    6.103981e-05,
    5.449395e-05,
    4.6121422e-05,
    3.0420308e-05,
    2.4557441e-05,
    1.882719e-05,
    1.2036036e-05,
    1.1538399e-05
];

static KICK_MASK: &[f32] = &[
    0.819422,
    1.0,
    0.75496507,
    0.5369861,
    0.32922322,
    0.19322844,
    0.13659032,
    0.105890565,
    0.09578366,
    0.07660119,
    0.06796505,
    0.05891491,
    0.050092228,
    0.04606803,
    0.043227687,
    0.037425213,
    0.033232316,
    0.028685851,
    0.02807352,
    0.027293457,
    0.0228076,
    0.018380264,
    0.014364035,
    0.012841125,
    0.012799092,
    0.010214381,
    0.008064171,
    0.0071528596,
    0.0062199086,
    0.0051540076,
    0.0038745347,
    0.004031128,
    0.004228361,
    0.0035796408,
    0.002805041,
    0.002248501,
    0.0019979437,
    0.001941717,
    0.0017692127,
    0.0018128176,
    0.00094725064,
    0.0011951958,
    0.000777927,
    0.0005237828,
    0.00073082204,
    0.0007187554,
    0.00064344844,
    0.00050969864,
    0.0003932768,
    0.00035244078,
    0.00026491078,
    0.00027480288,
    0.00027506333,
    0.00019471704,
    9.89444e-05,
    7.5499855e-05,
    9.615483e-05,
    8.822229e-05,
    7.966764e-05,
    5.159704e-05,
    3.9844224e-05,
    4.5039415e-05,
    3.526646e-05,
    2.2655602e-05,
    1.658063e-05,
    1.946495e-05,
    1.6671018e-05,
    9.996639e-06,
    7.3912834e-06,
    7.051342e-06,
    4.9961855e-06,
    3.4610844e-06,
    3.1166426e-06,
    2.2646961e-06,
    2.7993876e-06,
    1.5540812e-06,
    1.7957288e-06,
    1.0655147e-06,
    7.464788e-07,
    7.235864e-07,
    5.247165e-07,
    5.3780053e-07
    ];

static HIHAT_MASK: &[f32] = &[
    0.25170618,
    0.15444331,
    0.19990039,
    0.2656652,
    0.3348828,
    0.36928213,
    0.45698786,
    0.5218832,
    0.645029,
    0.8078512,
    0.7437648,
    0.71053636,
    0.7903703,
    0.7365646,
    0.67574984,
    0.63135314,
    0.64934194,
    0.7921627,
    0.8618746,
    0.96269554,
    1.0,
    0.7607025,
    0.55753934,
    0.54396075,
    0.5751719,
    0.53608733,
    0.54072714,
    0.55539644,
    0.4796334,
    0.40054247,
    0.34101316,
    0.3411985,
    0.28274712,
    0.28396133,
    0.3117013,
    0.29220772,
    0.2688066,
    0.31433,
    0.32402468,
    0.29516023,
    0.26301986,
    0.24923907,
    0.21512789,
    0.2122147,
    0.25603205,
    0.20741211,
    0.1950283,
    0.21562706,
    0.22354695,
    0.17941889,
    0.11878,
    0.11863568,
    0.1379137,
    0.128538,
    0.09066259,
    0.084438585,
    0.07660452,
    0.06062959,
    0.05324049,
    0.051020917,
    0.04948296,
    0.046838008,
    0.03541012,
    0.027945776,
    0.02382026,
    0.022108482,
    0.01952932,
    0.017931424,
    0.014907217,
    0.012170651,
    0.010639897,
    0.008134646,
    0.0076316986,
    0.006742377,
    0.006344368,
    0.0055395863,
    0.0042859865,
    0.0030542852,
    0.0026213042,
    0.0025193589,
    0.0021465106,
    0.0019510848
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
            adaptive_threshold: 0.4,
            mean_range: 5,
            ..Default::default()
        });

        let hihat = Advanced::with_settings(AdvancedSettings {
            fixed_threshold: 2.0,
            adaptive_threshold: 0.3,
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

        let drum_weight: f32 = diff.clone().zip(KICK_MASK).map(|(d, &w)| d * w).sum();

        let hihat_weight: f32 = diff.clone().zip(HIHAT_MASK).map(|(d, &w)| d * w).sum();

        let note_weight: f32 = diff.clone().zip(SNARE_MASK).map(|(d, &w)| d * w).sum();

        let onset = self.threshold.full.is_above(weight);

        let index_of_max = freq_bins
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        lightservices.event_detected(Event::Raw(hihat_weight));

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
