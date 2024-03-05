mod utils;

use std::error::Error;

use crate::utils::audiodevices::{create_monitor_stream, get_output_devices};
use crate::utils::config::{Config, ConfigError};
use log::{debug, error, info, warn};

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Warn)
        .parse_default_env()
        .init();

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

            return;
        }
    };

    let lightservices = match config.initialize_lightservices().await {
        Ok(vec) => vec,
        Err(e) => {
            error!("{e}");
            if let Some(e) = e.source() {
                debug!("{}", e);
            }
            return;
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
                    error!("Device not found: {}", config.audio_device);
                    warn!("Available devices:");
                    for name in get_output_devices() {
                        warn!("{name}");
                    }
                }
                _ => {
                    debug!("{e}");
                    if let Some(e) = e.source() {
                        debug!("{e}");
                    }
                }
            };
            return;
        }
    };

    println!("Stop sync with CTRL-C");

    tokio::signal::ctrl_c()
        .await
        .expect("Error setting Ctrl-C handler");

    info!("Shutting down");
    drop(stream);
    info!("Shutdown complete");
}
