mod utils;

use std::sync::mpsc::channel;

use cpal::{traits::StreamTrait};
use crate::utils::audiodevices::create_default_output_stream;


#[tokio::main]
async fn main() {
    let stream = create_default_output_stream();
    stream.play().unwrap();
    let (tx, rx) = channel();
    
    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    
    println!("Stop sync with CTRL-C");
    rx.recv().expect("Could not receive from channel.");
    println!("Shutting down"); 
}