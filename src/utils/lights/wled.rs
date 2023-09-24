use std::{
    sync::{Arc, Mutex},
    time::Duration, collections::VecDeque,
};

use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;

use crate::utils::audioprocessing::MelFilterBank;

use super::{
    envelope::{DynamicDecay, Envelope, FixedDecay},
    Onset, OnsetConsumer, Pollable, PollingHelper,
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LEDStrip {
    name: String,
    led_count: u16,
    ip: String,
    port: u16,
    segments: Vec<Segment>,
    rgbw: bool,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct LEDStripOnset {
    strip: LEDStrip,
    polling_helper: PollingHelper,
    state: Arc<Mutex<OnsetState>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct Segment {
    start: usize,
    stop: usize,
}

#[derive(Debug)]
struct OnsetState {
    led_count: u16,
    brightness: f64,
    rgbw: bool,
    drum_envelope: DynamicDecay,
    note_envelope: DynamicDecay,
    hihat_envelope: FixedDecay,
    prefix: Vec<u8>,
}

impl OnsetState {
    pub fn init(led_count: u16, rgbw: bool, brightness: f64) -> Self {
        let prefix = if rgbw {
            vec![0x03, 0x01]
        } else {
            vec![0x02, 0x01]
        };
        OnsetState {
            led_count,
            rgbw,
            drum_envelope: DynamicDecay::init(2.0),
            note_envelope: DynamicDecay::init(4.0),
            hihat_envelope: FixedDecay::init(Duration::from_millis(200)),
            prefix,
            brightness,
        }
    }
}

impl Pollable for OnsetState {
    fn poll(&self) -> Bytes {
        let channels = 3 + usize::from(self.rgbw);
        let mut bytes = BytesMut::with_capacity(2 + self.led_count as usize * channels);

        bytes.put_slice(&self.prefix);

        let red = self.drum_envelope.get_value() as f64 * self.led_count as f64 * 0.5;
        let blue = self.note_envelope.get_value() as f64 * self.led_count as f64 * 0.5;
        let white = self.hihat_envelope.get_value() as f64 * self.led_count as f64 * 0.2;

        let mut colors: Vec<Vec<u8>> = if self.rgbw {
            vec![vec![0, 0, 0, 0]; self.led_count as usize / 2]
        } else {
            vec![vec![0, 0, 0]; self.led_count as usize / 2]
        };

        for (i, color) in &mut colors.iter_mut().enumerate() {
            let r =
                ((red - i as f64).clamp(0.0, 1.0) * u8::MAX as f64 * self.brightness).round() as u8;
            let b = ((blue - i as f64).clamp(0.0, 1.0) * u8::MAX as f64 * self.brightness).round()
                as u8;
            let w = ((white - (self.led_count / 2 - i as u16) as f64).clamp(0.0, 1.0)
                * u8::MAX as f64
                * self.brightness)
                .round() as u8;

            if self.rgbw {
                *color = vec![r, 0, b, w];
            } else {
                *color = vec![r.saturating_add(w), w, b.saturating_add(w)];
            }
        }
        let mut reversed = colors.clone();
        reversed.reverse();
        reversed.extend(colors);
        for colors in reversed {
            bytes.put_slice(&colors);
        }

        bytes.into()
    }
}

impl LEDStripOnset {
    pub async fn connect(ip: &str) -> Result<LEDStripOnset, Box<dyn std::error::Error>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Leds {
            count: u16,
            rgbw: bool,
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Info {
            name: String,
            udpport: u16,
            leds: Leds,
            ver: String,
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        let url = format!("http://{}/json/info", ip);
        let resp = client.get(&url).send().await?;
        let info: Info = resp.json().await?;
        println!("Found strip {}", info.name);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let state = OnsetState::init(info.leds.count, info.leds.rgbw, 1.0);

        let state = Arc::new(Mutex::new(state));

        let polling_helper = PollingHelper::init(socket, state.clone(), 30);

        Ok(LEDStripOnset {
            strip: LEDStrip {
                name: info.name,
                led_count: info.leds.count,
                ip: ip.to_string(),
                port: info.udpport,
                segments: vec![Segment {
                    start: 0,
                    stop: info.leds.count as usize,
                }],
                rgbw: info.leds.rgbw,
            },
            polling_helper,
            state,
        })
    }
}

impl OnsetConsumer for LEDStripOnset {
    fn onset_detected(&mut self, event: Onset) {
        let mut state = self.state.lock().unwrap();
        match event {
            Onset::Drum(strength) => {
                state.drum_envelope.trigger(strength);
            }
            Onset::Hihat(strength) => {
                state.hihat_envelope.trigger(strength);
            }
            Onset::Note(strength, _) => {
                state.note_envelope.trigger(strength);
            }
            _ => {}
        };
    }

    fn update(&mut self) {
        // self updating
    }
}

pub struct LEDStripSpectrum {
    strip: LEDStrip,
    polling_helper: PollingHelper,
    state: Arc<Mutex<SpectrumState>>,
}

impl LEDStripSpectrum {
    pub async fn connect(ip: &str) -> Result<Self, Box<dyn std::error::Error>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Leds {
            count: u16,
            rgbw: bool,
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Info {
            name: String,
            udpport: u16,
            leds: Leds,
            ver: String,
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()?;
        let url = format!("http://{}/json/info", ip);
        let resp = client.get(&url).send().await?;
        let info: Info = resp.json().await?;
        println!("Found strip {}", info.name);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let state = SpectrumState::init(info.leds.count,  1.0, 1);

        let state = Arc::new(Mutex::new(state));

        let polling_helper = PollingHelper::init(socket, state.clone(), 50);

        Ok(LEDStripSpectrum {
            strip: LEDStrip {
                name: info.name,
                led_count: info.leds.count,
                ip: ip.to_string(),
                port: info.udpport,
                segments: vec![Segment {
                    start: 0,
                    stop: info.leds.count as usize,
                }],
                rgbw: info.leds.rgbw,
            },
            polling_helper,
            state,
        })
    }

    pub fn process_spectrum(&mut self, freq_bins: &[f32]) {
        let mut state = self.state.lock().unwrap();
        state.visualize_spectrum(freq_bins);
    }
}

pub struct SpectrumState {
    colors: VecDeque<[u8; 3]>,
    prefix: Vec<u8>,
    led_count: u16,
    brightness: f64,
    aggregate: u8,
    aggregate_count: u8,
}

impl SpectrumState {
    pub fn init(led_count: u16, brightness: f64, aggregate: u8) -> Self {
        let prefix = vec![0x02, 0x01];
        Self { 
            colors: VecDeque::from(vec![[0, 0, 0]; led_count as usize]), 
            prefix, 
            led_count, 
            brightness,
            aggregate,
            aggregate_count: 0,
        }
    }

    pub fn visualize_spectrum(&mut self, freq_bins: &[f32]) {
        self.aggregate_count += 1;

        let low_weight: f32 = freq_bins.iter()
            .take(10)
            .sum();
        let mid_weight: f32 = freq_bins.iter()
            .skip(10)
            .take(90)
            .sum();
        let highs_weight: f32 = freq_bins.iter()
            .skip(100)
            .sum();

        let max = low_weight.max(mid_weight.max(highs_weight));

        let [r, g, b] = [(low_weight / max * 255.0) as u8, (mid_weight / max * 255.0) as u8, (highs_weight / max * 255.0) as u8];

        if self.aggregate_count == self.aggregate {
            self.colors.pop_back();
            self.colors.push_front([r, g, b]);
            self.aggregate_count = 0;
        } else {
            let front = self.colors.front_mut().unwrap();
            *front = [
                (front[0] * self.aggregate_count + r) / self.aggregate, 
                (front[1] * self.aggregate_count + g) / self.aggregate, 
                (front[2] * self.aggregate_count + b) / self.aggregate,
            ];
        }
    }
}

impl Pollable for SpectrumState {
    fn poll(&self) -> Bytes {
        let mut bytes = BytesMut::with_capacity(2 + self.led_count as usize * 3);
        bytes.put_slice(&self.prefix);

        for color in &self.colors {
            bytes.put_slice(color);
        }

        bytes.into()
    }
}