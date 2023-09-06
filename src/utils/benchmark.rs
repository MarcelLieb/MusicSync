use std::{fs::File, io::BufReader};

use realfft::RealFftPlanner;
use rodio::{Decoder, Source};

use super::{
    audioprocessing::{hfc::Hfc, prepare_buffers, process_raw, ProcessingSettings},
    lights::{LightService, serialize},
};

pub fn process_file(filename: String, settings: ProcessingSettings) {
    let file = BufReader::new(File::open(filename.clone()).unwrap());

    let source = Decoder::new(file).unwrap();

    let serializer = serialize::OnsetContainer::init(
        filename.split('.').next().unwrap().to_owned() + ".cbor",
        settings.sample_rate as usize,
        settings.hop_size,
    );

    let channels = source.channels();
    let sample_rate = source.sample_rate();

    let ProcessingSettings {
        buffer_size,
        hop_size,
        fft_size,
        ..
    } = settings;

    let buffer_size = buffer_size * channels as usize;
    let hop_size = hop_size * channels as usize;

    let mut hfc = Hfc::init(sample_rate as usize, fft_size);

    let mut lightservices: Vec<Box<dyn LightService + Send>> = vec![Box::new(serializer)];

    let mut buffer_detection = prepare_buffers(channels, &settings);
    let fft_planner = RealFftPlanner::<f32>::new().plan_fft_forward(fft_size);
    let samples: Vec<f32> = source.convert_samples().collect();

    let n = samples.len() / hop_size;

    (0..n).for_each(|i| {
        let (peak, rms) = process_raw(
            &samples[i * hop_size..buffer_size + i * hop_size],
            channels,
            &fft_planner,
            &mut buffer_detection,
        );
        hfc.detect(&buffer_detection.freq_bins, peak, rms, &mut lightservices);
    });
}
