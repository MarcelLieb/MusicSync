use crate::utils::lights::{LightService, Onset};

use super::{
    threshold::{Advanced, AdvancedSettings},
    MelFilterBank, MelFilterBankSettings, OnsetDetector,
};

static SNARE_MASK: &[f32] = &[
    0.2517875,
    0.40162945,
    0.6608701,
    0.895559,
    1.0,
    0.9941745,
    0.89297336,
    0.77627707,
    0.68870044,
    0.6062603,
    0.55628353,
    0.46556687,
    0.44126529,
    0.41894165,
    0.42049137,
    0.41696343,
    0.41548678,
    0.44371694,
    0.47437596,
    0.44820136,
    0.40240806,
    0.3569819,
    0.33027217,
    0.33291867,
    0.2950501,
    0.2666286,
    0.23969601,
    0.24532156,
    0.23413229,
    0.1845304,
    0.15278703,
    0.14658077,
    0.12533043,
    0.12600358,
    0.12842804,
    0.10343659,
    0.080343366,
    0.085119955,
    0.08721107,
    0.072455004,
    0.06042953,
    0.054964475,
    0.04232392,
    0.037744675,
    0.04250465,
    0.033505596,
    0.029123895,
    0.02615134,
    0.025658138,
    0.022041397,
    0.014772082,
    0.013949036,
    0.015307835,
    0.011997595,
    0.00960505,
    0.0074932203,
    0.0073791686,
    0.006116907,
    0.0046843905,
    0.0035925682,
    0.0033158308,
    0.0036108983,
    0.0026803834,
    0.0022112355,
    0.0018353146,
    0.0020247328,
    0.0014468391,
    0.0011526725,
    0.0010442777,
    0.0009913357,
    0.0006724108,
    0.00056849775,
    0.0004546819,
    0.00034425044,
    0.0002761672,
    0.00024362215,
    0.00019974195,
    0.00013049744,
    0.0001049808,
    8.013556e-05,
    5.3011227e-05,
    5.1095893e-05,
];

static KICK_MASK: &[f32] = &[
    0.92297435,
    1.0,
    0.8917986,
    0.76588273,
    0.5980251,
    0.43760094,
    0.34817106,
    0.29070562,
    0.26999202,
    0.22773525,
    0.20728031,
    0.18475792,
    0.16161652,
    0.15063743,
    0.14271763,
    0.12607518,
    0.11363953,
    0.09974024,
    0.097833805,
    0.095392935,
    0.08108377,
    0.06648338,
    0.05280021,
    0.047496855,
    0.047349576,
    0.038194574,
    0.030428723,
    0.027094975,
    0.023655336,
    0.019691972,
    0.0148861585,
    0.0154771805,
    0.01622049,
    0.013770917,
    0.010827841,
    0.008700853,
    0.007739898,
    0.0075239544,
    0.006860751,
    0.007028481,
    0.0036867769,
    0.004646642,
    0.0030300743,
    0.002042459,
    0.0028471632,
    0.002800305,
    0.0025077746,
    0.0019876803,
    0.0015344466,
    0.0013753675,
    0.0010342064,
    0.0010727863,
    0.0010738191,
    0.0007604331,
    0.00038656426,
    0.0002949998,
    0.00037564998,
    0.000344716,
    0.00031129658,
    0.00020160488,
    0.00015570206,
    0.00017598891,
    0.00013784677,
    8.852099e-05,
    6.481363e-05,
    7.609808e-05,
    6.517598e-05,
    3.908605e-05,
    2.8887778e-05,
    2.7541795e-05,
    1.9517582e-05,
    1.3512265e-05,
    1.2166234e-05,
    8.8529105e-06,
    1.0923741e-05,
    6.057274e-06,
    7.040926e-06,
    4.141735e-06,
    2.8992185e-06,
    2.847447e-06,
    2.0708724e-06,
    2.122644e-06,
];

static HIHAT_MASK: &[f32] = &[
    0.5139305,
    0.37839606,
    0.44701898,
    0.5303261,
    0.6035715,
    0.63583946,
    0.70866656,
    0.75564235,
    0.8328448,
    0.9175402,
    0.8861456,
    0.8689281,
    0.9091993,
    0.8824723,
    0.8501405,
    0.8249181,
    0.8353149,
    0.91006196,
    0.94234896,
    0.98516864,
    1.0,
    0.8946662,
    0.77944165,
    0.7705315,
    0.79074574,
    0.76528,
    0.7683824,
    0.7780476,
    0.7256422,
    0.66321856,
    0.60950035,
    0.6096783,
    0.5495825,
    0.5509193,
    0.5803903,
    0.55989105,
    0.5339321,
    0.583082,
    0.5928673,
    0.56305873,
    0.5272662,
    0.5109673,
    0.4677784,
    0.46388286,
    0.51907796,
    0.45738363,
    0.4401618,
    0.4684424,
    0.47884384,
    0.4174335,
    0.3157647,
    0.31549197,
    0.35048515,
    0.33381835,
    0.25910708,
    0.24549969,
    0.22774297,
    0.18911576,
    0.17001644,
    0.16411132,
    0.159972,
    0.15275972,
    0.12014305,
    0.097435035,
    0.084355466,
    0.078810595,
    0.0703201,
    0.06497539,
    0.054676216,
    0.045141328,
    0.039715063,
    0.030685436,
    0.028849835,
    0.02558493,
    0.024115803,
    0.021129861,
    0.016437387,
    0.011777007,
    0.010126779,
    0.009737301,
    0.008309941,
    0.007559927,
];

pub struct SpecFlux {
    filter_bank: MelFilterBank,
    old_spectrum: Vec<f32>,
    spectrum: Vec<f32>,
    threshold: ThresholdBank,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpecFluxSettings {
    pub filter_bank_settings: MelFilterBankSettings,
    pub threshold_bank_settings: ThresholdBankSettings,
}

#[derive(Debug, Clone, Copy)]
pub struct ThresholdBankSettings {
    pub drum: AdvancedSettings,
    pub hihat: AdvancedSettings,
    pub note: AdvancedSettings,
    pub full: AdvancedSettings,
}

impl Default for ThresholdBankSettings {
    fn default() -> Self {
        Self {
            drum: AdvancedSettings {
                fixed_threshold: 2.0,
                dynamic_threshold: 0.4,
                mean_range: 5,
                ..Default::default()
            },
            hihat: AdvancedSettings {
                fixed_threshold: 5.0,
                dynamic_threshold: 0.55,
                mean_range: 3,
                ..Default::default()
            },
            note: AdvancedSettings {
                fixed_threshold: 2.0,
                dynamic_threshold: 0.4,
                ..Default::default()
            },
            full: AdvancedSettings::default(),
        }
    }
}

struct ThresholdBank {
    drum: Advanced,
    hihat: Advanced,
    note: Advanced,
    full: Advanced,
}

impl ThresholdBank {
    pub fn with_settings(settings: ThresholdBankSettings) -> Self {
        Self {
            drum: Advanced::with_settings(settings.drum),
            hihat: Advanced::with_settings(settings.hihat),
            note: Advanced::with_settings(settings.note),
            full: Advanced::with_settings(settings.full),
        }
    }
}

impl Default for ThresholdBank {
    fn default() -> Self {
        ThresholdBank::with_settings(ThresholdBankSettings::default())
    }
}

impl SpecFlux {
    pub fn init(sample_rate: u32, fft_size: u32) -> Self {
        let bands = MelFilterBankSettings::default().bands;
        let bank =
            MelFilterBank::with_settings(sample_rate, fft_size, MelFilterBankSettings::default());
        let threshold = ThresholdBank::default();
        let spectrum = vec![0.0; bands];
        let old_spectrum = vec![0.0; bands];
        Self {
            filter_bank: bank,
            spectrum,
            old_spectrum,
            threshold,
        }
    }

    pub fn with_settings(sample_rate: u32, fft_size: u32, settings: SpecFluxSettings) -> Self {
        let bank =
            MelFilterBank::with_settings(sample_rate, fft_size, settings.filter_bank_settings);
        let threshold = ThresholdBank::with_settings(settings.threshold_bank_settings);
        let spectrum = vec![0.0; settings.filter_bank_settings.bands];
        let old_spectrum = vec![0.0; settings.filter_bank_settings.bands];
        Self {
            filter_bank: bank,
            old_spectrum,
            spectrum,
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
        self.old_spectrum.clone_from(&self.spectrum);

        let lambda = 0.1;

        self.filter_bank.filter(freq_bins, &mut self.spectrum);

        self.spectrum
            .iter_mut()
            .for_each(|x| *x = (*x * lambda).ln_1p());

        let flux = self
            .old_spectrum
            .iter()
            .zip(&self.spectrum)
            .map(|(&a, &b)| (((b - a) + (b - a).abs()) / 2.0));

        let weight: f32 = flux.clone().sum();

        let drum_weight: f32 = flux.clone().zip(KICK_MASK).map(|(d, &w)| d * w).sum();

        let hihat_weight: f32 = flux.clone().zip(HIHAT_MASK).map(|(d, &w)| d * w).sum();

        let note_weight: f32 = flux.clone().zip(SNARE_MASK).map(|(d, &w)| d * w).sum();

        let onset = self.threshold.full.is_above(weight);

        let index_of_max = freq_bins
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .unwrap()
            .0;

        lightservices.onset_detected(Onset::Raw(hihat_weight));

        if onset {
            lightservices.onset_detected(Onset::Full(rms));
        }

        if self.threshold.drum.is_above(drum_weight) {
            lightservices.onset_detected(Onset::Drum(rms));
        }

        if self.threshold.hihat.is_above(hihat_weight) {
            lightservices.onset_detected(Onset::Hihat(peak));
        }

        if self.threshold.note.is_above(note_weight) {
            lightservices.onset_detected(Onset::Note(rms, index_of_max as u16));
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
