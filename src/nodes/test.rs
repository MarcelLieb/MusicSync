use crate::nodes::{audio::fft::FFT, DataGraphManager, PrintNode};

use super::{audio::{capture::LoopbackNode, filterbank::MelFilterBankNode}, general::array::Window};

pub fn test_chain(secs: u64) {
    let mut graph_manager = DataGraphManager::new();
    let audio_input = LoopbackNode::new(&graph_manager, "", 48_000).unwrap();
    let sliding_window = Window::init(4096, 480);
    let fft = FFT::init(4096, crate::utils::audioprocessing::WindowType::Hann);
    let mel_filter_bank = MelFilterBankNode::new(10, 4096, 44100, 0.0, 22050.0);
    let printer = PrintNode::new(1);
    println!("Done creating nodes");

    let input_id = audio_input.id.clone();
    let window_id = graph_manager.add_node(sliding_window);
    let fft_id = graph_manager.add_node(fft);
    let mel_filter_bank_id = graph_manager.add_node(mel_filter_bank);
    let printer_id = graph_manager.add_node(printer);
    println!("Done adding nodes");

    graph_manager.follow(&input_id, &window_id, 0, 0);
    graph_manager.follow(&window_id, &fft_id, 0, 0);
    graph_manager.follow(&fft_id, &mel_filter_bank_id, 0, 0);
    graph_manager.follow(&mel_filter_bank_id, &printer_id, 0, 0);

    println!("Done following nodes");

    std::thread::sleep(std::time::Duration::from_secs(secs));
}
