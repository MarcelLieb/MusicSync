use std::{fmt::Display, fs};

use serde::{Deserialize, Serialize};

use super::{audioprocessing::{hfc::HfcSettings, spectral_flux::SpecFluxSettings, ProcessingSettings}, lights::{hue::HueSettings, wled::{OnsetSettings, SpectrumSettings}, LightService}};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    audio_device: String,
    console_output: bool,
    serialize_data: bool,
    audio_processing: ProcessingSettings, 
    onset_detector: OnsetDetector,
    hue: Vec<HueSettings>,
    wled_spectrum: Vec<SpectrumSettings>,
    wled_onset: Vec<OnsetSettings>,
}

#[derive(Debug)]
pub enum ConfigError {
    File(std::io::Error),
    Parse(toml::de::Error),
}

impl From<std::io::Error> for ConfigError {
    fn from(value: std::io::Error) -> Self {
        Self::File(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::Parse(value)
    }
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(_) => write!(f, "Config file not found"),
            Self::Parse(_) => write!(f, "Parsing config failed"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::File(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum OnsetDetector {
    SpecFlux(SpecFluxSettings),
    HFC(HfcSettings),
}

impl Default for OnsetDetector {
    fn default() -> Self {
        Self::SpecFlux(SpecFluxSettings::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { 
            audio_device: "".to_owned(), 
            console_output: true, 
            serialize_data: false, 
            audio_processing: ProcessingSettings::default(), 
            onset_detector: OnsetDetector::default(), 
            hue: Vec::new(), 
            wled_spectrum: Vec::new(), 
            wled_onset: Vec::new(),
        }
    }
}

pub fn load_config(file: &str) -> Result<Config, ConfigError> {
    let contents = fs::read_to_string(file)?;
    
    Ok(toml::de::from_str(&contents)?)
}