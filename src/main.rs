use cpal::{self, traits::{HostTrait, DeviceTrait}, InputCallbackInfo};

fn main() {
    let _hosts = cpal::available_hosts();
    let default_host = cpal::default_host();
    
    let default_output = default_host.default_output_device().unwrap();
    let audio_cfg = default_output
    .default_output_config()
    .expect("No default output config found");

    default_output.build_input_stream(&audio_cfg.config(), |_:&[i32], _:&InputCallbackInfo| {}, |_| {} , None);

    println!("Hello, world!");
}
