use std::{error::Error, fmt::Display, fs, net::Ipv4Addr};

use serde::{Deserialize, Serialize};

use super::{
    audioprocessing::{
        self,
        hfc::{Hfc, HfcSettings},
        spectral_flux::{SpecFlux, SpecFluxSettings},
        ProcessingSettings,
    },
    lights::{
        console::Console,
        hue::{self, HueError, HueSettings},
        serialize,
        wled::{self, OnsetSettings, SpectrumSettings, WLEDError},
        LightService,
    },
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    #[serde(default, rename = "audio_device")]
    pub audio_device: String,

    #[serde(default, rename = "console_output")]
    pub console_output: bool,

    #[serde(default, rename = "serialize_onsets")]
    pub serialize_onsets: Option<String>,

    #[serde(default, rename = "Audio")]
    pub audio_processing: ProcessingSettings,

    #[serde(default)]
    pub onset_detector: OnsetDetector,

    #[serde(default)]
    pub hue: Vec<HueSettings>,

    #[serde(default, rename = "WLED")]
    pub wled: Vec<WLEDConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "effect")]
pub enum WLEDConfig {
    Spectrum {
        ip: String,
        #[serde(default, flatten)]
        settings: SpectrumSettings,
    },
    Onset {
        ip: String,
        #[serde(default, flatten)]
        settings: OnsetSettings,
    },
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
            Self::FileFormat => write!(f, "Config file must end in '.toml'"),
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
#[serde(tag = "algorithm")]
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
            console_output: false,
            serialize_onsets: None,
            audio_processing: ProcessingSettings::default(),
            onset_detector: OnsetDetector::default(),
            hue: Vec::new(),
            wled: Vec::new(),
        }
    }
}

impl Config {
    pub fn load(file: &str) -> Result<Self, ConfigError> {
        if file.split_terminator(".").last() != Some("toml") {
            return Err(ConfigError::FileFormat);
        }

        let contents = fs::read_to_string(file)?;

        Ok(toml::de::from_str(&contents)?)
    }

    pub async fn initialize_lightservices(
        &self,
    ) -> Result<Vec<Box<dyn LightService + Send>>, LightServiceError> {
        let mut lightservices: Vec<Box<dyn LightService + Send>> = Vec::new();

        if let Some(path) = &self.serialize_onsets {
            let path = if path == "" { "onsets.cbor" } else { path };
            let serializer = serialize::OnsetContainer::init(
                path,
                self.audio_processing.sample_rate as usize,
                self.audio_processing.hop_size,
            );
            lightservices.push(Box::new(serializer))
        }

        if self.console_output {
            let console = Console::default();
            lightservices.push(Box::new(console));
        }

        for config in &self.wled {
            match config {
                WLEDConfig::Spectrum { ip, settings } => {
                    let strip = wled::LEDStripSpectrum::connect_with_settings(
                        ip,
                        self.audio_processing.sample_rate as f32,
                        *settings,
                    )
                    .await?;
                    lightservices.push(Box::new(strip));
                }
                WLEDConfig::Onset { ip, settings } => {
                    let strip = wled::LEDStripOnset::connect_with_settings(ip, *settings).await?;
                    lightservices.push(Box::new(strip));
                }
            }
        }

        for settings in &self.hue {
            let bridge = hue::connect_with_settings(settings.clone()).await?;
            lightservices.push(Box::new(bridge));
        }

        Ok(lightservices)
    }

    pub fn initialize_onset_detector(
        &self,
    ) -> Box<dyn audioprocessing::OnsetDetector + Send + 'static> {
        let detector: Box<dyn audioprocessing::OnsetDetector + Send + 'static>;
        match self.onset_detector {
            OnsetDetector::SpecFlux(settings) => {
                let alg = SpecFlux::with_settings(
                    self.audio_processing.sample_rate,
                    self.audio_processing.fft_size as u32,
                    settings,
                );
                detector = Box::new(alg);
            }
            OnsetDetector::HFC(settings) => {
                let alg = Hfc::with_settings(
                    self.audio_processing.sample_rate as usize,
                    self.audio_processing.fft_size,
                    settings,
                );
                detector = Box::new(alg);
            }
        };
        detector
    }

    #[allow(dead_code)]
    pub fn generate_template(file_path: &str) {
        let mut template = Config::default();
        template.onset_detector = OnsetDetector::SpecFlux(Default::default());
        template.wled.push(WLEDConfig::Spectrum {
            ip: "Ip of Strip".to_owned(),
            settings: Default::default(),
        });
        template.wled.push(WLEDConfig::Onset {
            ip: "Ip of Strip".to_owned(),
            settings: Default::default(),
        });
        template.hue.push(HueSettings {
            ip: Some(Ipv4Addr::new(0, 0, 0, 0)),
            area: Some("Area uuid".to_owned()),
            ..Default::default()
        });
        let toml = toml::to_string(&template).unwrap();
        fs::write(file_path, toml).unwrap();
    }
}

#[derive(Debug)]
pub enum LightServiceError {
    Hue(HueError),
    WLED(WLEDError),
}

impl From<HueError> for LightServiceError {
    fn from(value: HueError) -> Self {
        Self::Hue(value)
    }
}

impl From<WLEDError> for LightServiceError {
    fn from(value: WLEDError) -> Self {
        Self::WLED(value)
    }
}

impl std::error::Error for LightServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            LightServiceError::Hue(e) => Some(e),
            LightServiceError::WLED(e) => Some(e),
        }
    }
}

impl Display for LightServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LightServiceError::Hue(_) => write!(f, "Connection to bridge failed"),
            LightServiceError::WLED(_) => write!(f, "Connection to WLED strip failed"),
        }
    }
}
