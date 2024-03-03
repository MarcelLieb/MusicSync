mod utils;

use std::error::Error;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::utils::audiodevices::create_monitor_stream;
use crate::utils::config::{Config, ConfigError};
use log::{debug, error};
use tokio::time::sleep;

#[tokio::main]
async fn main() {
    let config = match Config::load("./config.toml") {
        Ok(loaded_config) => loaded_config,
        Err(e) => {
            error!("Error loading config");
            if let ConfigError::Parse(e) = &e {
                error!("{e}");
            } else {
                debug!("{e}");
                if let Some(e) = e.source() {
                    debug!("{e}");
                }
            }

            error!("Using default config");
            Config::default()
        }
    };

    let lightservices = match config.initialize_lightservices().await {
        Ok(vec) => vec,
        Err(e) => {
            error!("{e}");
            if let Some(e) = e.source() {
                debug!("{}", e);
            }
            panic!("Couldn't initialize Lightservices");
        }
    };

    let onset_detector = config.initialize_onset_detector();

    let stream = match create_monitor_stream(
        &config.audio_device,
        config.audio_processing,
        onset_detector,
        lightservices,
    ) {
        Ok(stream) => stream,
        Err(e) => {
            match e {
                cpal::BuildStreamError::DeviceNotAvailable => {
                    error!("Device not found: {}", config.audio_device)
                }
                _ => {
                    debug!("{e}");
                    if let Some(e) = e.source() {
                        debug!("{e}");
                    }
                }
            };
            panic!("Audio stream couldn't be build");
        }
    };

    let (tx, rx) = channel();

    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");

    println!("Stop sync with CTRL-C");
    rx.recv().expect("Could not receive from channel.");
    println!("Shutting down");
    drop(stream);
    // Wait for proper shutdown
    sleep(Duration::from_millis(100)).await;
}
