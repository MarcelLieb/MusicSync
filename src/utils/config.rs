use std::{fmt::Display, fs};

use serde::{Deserialize, Serialize};

use super::{audioprocessing::{hfc::HfcSettings, spectral_flux::SpecFluxSettings, ProcessingSettings}, lights::{hue::HueSettings, wled::{OnsetSettings, SpectrumSettings}}};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub audio_device: String,
    pub console_output: bool,
    pub serialize_data: bool,
    pub audio_processing: ProcessingSettings, 
    pub onset_detector: OnsetDetector,
    pub hue: Vec<HueSettings>,
    pub wled: Vec<WLEDConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WLEDConfig {
    Spectrum{ip: String, settings: SpectrumSettings},
    Onset{ip: String, settings: OnsetSettings}
}

#[derive(Debug)]
pub enum ConfigError {
    File(std::io::Error),
    FileFormat,
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
            Self::FileFormat => write!(f, "Config file must end in '.toml'")
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::File(e) => Some(e),
            ConfigError::Parse(e) => Some(e),
            ConfigError::FileFormat => None,
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
            wled: Vec::new(), 
        }
    }
}

pub fn load_config(file: &str) -> Result<Config, ConfigError> {
    if file.split_terminator(".").last() != Some("toml") {
        return Err(ConfigError::FileFormat);
    }
    
    let contents = fs::read_to_string(file)?;
    
    Ok(toml::de::from_str(&contents)?)
}