use std::{fs::File, io::BufReader};

use realfft::RealFftPlanner;
use rodio::{Decoder, Source};

use super::{
    audioprocessing::{prepare_buffers, process_raw, DetectionSettings, threshold::MultiBandThreshold, hfc::hfc},
    lights::LightService,
    serialize,
};

pub fn process_file(filename: String, settings: DetectionSettings) {
    let file = BufReader::new(File::open(filename.clone()).unwrap());

    let source = Decoder::new(file).unwrap();

    let serializer =
        serialize::OnsetContainer::init(filename.split(".").next().unwrap().to_owned() + ".cbor");

    let channels = source.channels();
    let sample_rate = source.sample_rate();

    let DetectionSettings {
        hop_size,
        buffer_size,
        threshold_settings,
        detection_weights,
    } = settings;
    let buffer_size = buffer_size * channels as usize;
    let hop_size = hop_size * channels as usize;

    let mut multi_threshold = MultiBandThreshold::init_settings(threshold_settings);
    let mut lightservices: Vec<Box<dyn LightService + Send>> = vec![Box::new(serializer)];

    let mut buffer_detection = prepare_buffers(channels, sample_rate);
    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(sample_rate as usize);
    let samples: Vec<f32> = source.convert_samples().collect();

    let n = samples.len() / hop_size;

    (0..n).for_each(|i| {
        let (peak, rms) = process_raw(
            &samples[i * hop_size..buffer_size + i * hop_size],
            channels,
            &fft_planner,
            &mut buffer_detection,
        );
        hfc(
            &buffer_detection.freq_bins, 
            peak,
            rms,
            &mut multi_threshold, 
            Some(&detection_weights), 
            &mut lightservices,
        );
    });
}
