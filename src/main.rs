mod utils;

use cpal::{traits::StreamTrait};
use crate::utils::audiodevices::create_default_output_stream;


#[tokio::main]
async fn main() {
    let stream = create_default_output_stream();
    stream.play().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(100));
}