use crate::{nodes::{audio::{spectral::FFT, onset, peak_picking}, DataGraphManager, PrintNode}, utils::audioprocessing::threshold};

use super::{audio::{capture::LoopbackNode, filterbank::MelFilterBankNode}, general::array::Window};

pub fn test_chain(secs: u64) {
    let mut graph_manager = DataGraphManager::new();
    let audio_input = LoopbackNode::new(&graph_manager, "", 48_000).unwrap();
    let sliding_window = Window::init(1024, 480);
    let fft = FFT::init(2048, crate::utils::audioprocessing::WindowType::Hann);
    let printer_fft = PrintNode::new(10000);
    let mel_filter_bank = MelFilterBankNode::new(82, 2048, 48000, 20.0, 20000.0);
    let spec_flux = onset::SpecFlux::init();
    let threshold = peak_picking::PeakPickingNode::new(threshold::BasicSettings::default());
    let printer_onset = PrintNode::new(1);
    println!("Done creating nodes");

    let input_id = audio_input.id.clone();
    let window_id = graph_manager.add_node(sliding_window);
    let fft_id = graph_manager.add_node(fft);
    let printer_fft_id = graph_manager.add_node(printer_fft);
    let mel_filter_bank_id = graph_manager.add_node(mel_filter_bank);
    let spec_flux_id = graph_manager.add_node(spec_flux);
    let threshold_id = graph_manager.add_node(threshold);
    let printer_onset_id = graph_manager.add_node(printer_onset);
    println!("Done adding nodes");

    graph_manager.follow(&input_id, &window_id, 0, 0);
    graph_manager.follow(&window_id, &fft_id, 0, 0);
    graph_manager.follow(&fft_id, &printer_fft_id, 0, 0);
    graph_manager.follow(&fft_id, &mel_filter_bank_id, 0, 0);
    graph_manager.follow(&mel_filter_bank_id, &spec_flux_id, 0, 0);
    graph_manager.follow(&spec_flux_id, &threshold_id, 0, 0);
    graph_manager.follow(&threshold_id, &printer_onset_id, 0, 0);

    println!("Done following nodes");

    // for i in 0..1_000_000 {
    //     graph_manager.handle_data((window_id.clone(), 0), crate::nodes::Data::FloatArray(vec![1.0; 4096].into()));
    //     if i % 1000 == 0 {
    //         std::thread::sleep(std::time::Duration::from_millis(100));
    //     }
    // }

    std::thread::sleep(std::time::Duration::from_secs(secs));
}
