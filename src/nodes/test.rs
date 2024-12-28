use std::sync::Arc;

use crate::nodes::{audio::fft::FFT, DataGraphManager, PrintNode};

use super::{
    audio::filterbank::MelFilterBankNode,
    general::array::Window, Data,
};

pub async fn test_chain(secs: u64) {
    let graph_manager = DataGraphManager::new();
    let window1: Window<f32> = Window::init(4096 * 4, 4096 * 4);
    let window2: Window<f32> = Window::init(4096, 4096);
    let window3: Window<f32> = Window::init(4096, 4096);
    let window4: Window<f32> = Window::init(4096, 4096);
    let window5: Window<f32> = Window::init(4096, 4096);
    let window6: Window<f32> = Window::init(4096, 4096);
    let window7: Window<f32> = Window::init(4096, 480);
    let window8: Window<f32> = Window::init(4096, 4096);
    let window9: Window<f32> = Window::init(4096, 1024);
    let window10: Window<f32> = Window::init(4096, 4096);
    let fft = FFT::init(4096, crate::utils::audioprocessing::WindowType::Hann);
    let mel_filter_bank = MelFilterBankNode::new(1000, 4096, 44100, 0.0, 22050.0);
    let printer = PrintNode::new(1_000);
    
    let window1_id = graph_manager.add_node(window1);
    let window2_id = graph_manager.add_node(window2);
    let window3_id = graph_manager.add_node(window3);
    let window4_id = graph_manager.add_node(window4);
    let window5_id = graph_manager.add_node(window5);
    let window6_id = graph_manager.add_node(window6);
    let window7_id = graph_manager.add_node(window7);
    let window8_id = graph_manager.add_node(window8);
    let window9_id = graph_manager.add_node(window9);
    let window10_id = graph_manager.add_node(window10);
    let fft_id = graph_manager.add_node(fft);
    let mel_filter_bank_id = graph_manager.add_node(mel_filter_bank);
    let printer_id = graph_manager.add_node(printer);

    graph_manager.follow(&window1_id, &window2_id, 0, 0);
    graph_manager.follow(&window2_id, &window3_id, 0, 0);
    graph_manager.follow(&window3_id, &window4_id, 0, 0);
    graph_manager.follow(&window4_id, &window5_id, 0, 0);
    graph_manager.follow(&window5_id, &window6_id, 0, 0);
    graph_manager.follow(&window6_id, &window7_id, 0, 0);
    graph_manager.follow(&window7_id, &window8_id, 0, 0);
    graph_manager.follow(&window8_id, &window9_id, 0, 0);
    graph_manager.follow(&window9_id, &window10_id, 0, 0);
    graph_manager.follow(&window10_id, &fft_id, 0, 0);
    graph_manager.follow(&fft_id, &mel_filter_bank_id, 0, 0);
    graph_manager.follow(&mel_filter_bank_id, &printer_id, 0, 0);

    for i in 0..100_000 {
        let data = Data::FloatArray(Arc::from(vec![1.0; 4096 * 4]));
        graph_manager.handle_data((window1_id.clone(), 0), data.clone());
        if i % 1000 == 0 {
            println!("Sent {} data points", i);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(secs));
}
