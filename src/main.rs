use cpal::{self, traits::{HostTrait, DeviceTrait}, InputCallbackInfo};

fn main() {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let default_output = default_host.default_output_device().unwrap();
    let audio_cfg = default_output
    .default_output_config()
    .expect("No default output config found");

    let _out = default_output.build_input_stream(&audio_cfg.config(), |data:&[i32], _:&InputCallbackInfo| print_data(data), |_| {} , None);

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    println!("Hello, world!");
}

fn print_data(data: &[i32]) {
    for i in data {
        println!("{}", i);
    }
}