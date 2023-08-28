use crate::utils::{lights::{LightService, Event}, audiodevices::SAMPLE_RATE};

use super::{threshold::AdvancedThreshold, MelFilterBank};

static SCALE: &'static[f32] = &[25.5, 29.14, 30.87, 32.70, 34.65, 36.71, 38.89, 41.20, 43.65, 46.25, 49.0, 51.91];
static FREQUENCIES: &'static[f32] = &[0.0, 25.5, 29.14, 30.87, 32.7, 34.65, 36.71, 38.89, 41.2, 43.65, 46.25, 49.0, 51.91, 51.0, 58.28, 61.74, 65.4, 69.3, 73.42, 77.78, 82.4, 87.3, 92.5, 98.0, 103.82, 102.0, 116.56, 123.48, 130.8, 138.6, 146.84, 155.56, 164.8, 174.6, 185.0, 196.0, 207.64, 204.0, 233.12, 246.96, 261.6, 277.2, 293.68, 311.12, 329.6, 349.2, 370.0, 392.0, 415.28, 408.0, 466.24, 493.92, 523.2, 554.4, 587.36, 622.24, 659.2, 698.4, 740.0, 784.0, 830.56, 816.0, 932.48, 987.84, 1046.4, 1108.8, 1174.72, 1244.48, 1318.4, 1396.8, 1480.0, 1568.0, 1661.12, 1632.0, 1864.96, 1975.68, 2092.8, 2217.6, 2349.44, 2488.96, 2636.8, 2793.6, 2960.0, 3136.0, 3322.24, 3264.0, 3729.92, 3951.36, 4185.6, 4435.2, 4698.88, 4977.92, 5273.6, 5587.2, 5920.0, 6272.0, 6644.48, 6528.0, 7459.84, 7902.72, 8371.2, 8870.4, 9397.76, 9955.84, 10547.2, 11174.4, 11840.0, 12544.0, 13288.96, 13056.0, 14919.68, 15805.44, 20000.0];

pub struct SpecFlux {
    filter_bank: MelFilterBank,
    spectrum: Vec<f32>,
    threshold: AdvancedThreshold
}

impl SpecFlux {
    pub fn init() -> SpecFlux{
        let bank = MelFilterBank::init(SAMPLE_RATE, SAMPLE_RATE, 82, 20_000);
        let threshold = AdvancedThreshold::init();
        let spectrum = Vec::with_capacity(82);
        SpecFlux { filter_bank: bank, spectrum, threshold }
    }

    pub fn detect(
        &mut self, 
        freq_bins: &Vec<f32>,
        peak: f32,
        rms: f32,
        lightservices: &mut [Box<dyn LightService + Send>], 
    ) {
        let old_spec = &self.spectrum;
        
        let lambda = 0.1;

        let mut spectrum: Vec<f32> = Vec::with_capacity(82);
        self.filter_bank.filter(freq_bins, &mut spectrum);

        spectrum.iter_mut().for_each(|x| *x = (*x * lambda).ln_1p());


        let weight: f32 = old_spec.iter().zip(&spectrum).map(|(&a, &b) | (((b - a) + (b - a).abs()) / 2.0)).sum();

        let onset = self.threshold.is_above(weight);

        lightservices
            .iter_mut()
            .for_each(|service| service.event_detected(Event::Raw(weight)));

        if onset {
            lightservices
                .iter_mut()
                .for_each(|service| service.event_detected(Event::Drum(rms)));
        }

        lightservices
            .iter_mut()
            .for_each(|service| service.update());

        self.spectrum = spectrum;
    }
}
