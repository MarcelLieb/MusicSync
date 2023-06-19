use std::{fs::File, io::BufReader};

use realfft::RealFftPlanner;
use rodio::{Decoder, Source};

use super::{
    audioprocessing::{print_onset, MultiBandThreshold, prepare_buffers, DetectionSettings},
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

    let DetectionSettings { hop_size, buffer_size, threshold_settings, detection_weights } = settings;
    let buffer_size = buffer_size * channels as usize;

    let mut multi_threshold = MultiBandThreshold::init_settings(threshold_settings);
    let mut lightservices: Vec<Box<dyn LightService + Send>> = vec![Box::new(serializer)];

    let mut buffer = prepare_buffers(channels, sample_rate);
    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(sample_rate as usize);
    let mut samples: Vec<f32> = source.convert_samples().collect();

    let n = samples.len() / hop_size;

    (0..n).for_each(|_| {
        print_onset(
            &samples[0..buffer_size],
            channels,
            &fft_planner,
            &mut buffer,
            &mut multi_threshold,
            &mut lightservices,
            Some(&detection_weights)
        );
        samples.drain(0..hop_size);
    });
}
