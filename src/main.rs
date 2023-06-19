mod utils;

use std::collections::HashMap;
use std::{fs::File, sync::mpsc::channel};

use crate::utils::audiodevices::create_default_output_stream;
use crate::utils::lights::Event;
use crate::utils::plot::plot;
use crate::utils::benchmark::process_file;
use ciborium::from_reader;
use cpal::traits::StreamTrait;
use utils::audioprocessing::DetectionSettings;

#[tokio::main]
async fn main() {
    {
        let stream = create_default_output_stream();
        stream.play().unwrap();
        let (tx, rx) = channel();

        ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
            .expect("Error setting Ctrl-C handler");

        println!("Stop sync with CTRL-C");
        rx.recv().expect("Could not receive from channel.");
        println!("Shutting down");
    }

    let file = File::open("onsets.cbor").expect("Couldn't open file");
    let data: HashMap<String, Vec<(u128, Event)>> = from_reader(file).unwrap();
    plot(&data, "plot.png".to_string()).unwrap();
    process_file("/home/marclie/Music/Twenty One Pilots - Heathens (Magnetude Cover).wav".to_string(), DetectionSettings::default());
}
