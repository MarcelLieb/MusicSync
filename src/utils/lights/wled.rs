use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type, Q_BUTTERWORTH_F32};
use bytes::{BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;

use super::{
    color::{color_downsample, color_upsample, hsv_to_rgb, rgb_to_hsv},
    envelope::{DynamicDecay, Envelope, FixedDecay},
    LightService, Onset, Pollable, PollingHelper,
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
    brightness: f32,
    rgbw: bool,
    drum_envelope: DynamicDecay,
    note_envelope: DynamicDecay,
    hihat_envelope: FixedDecay,
    prefix: Vec<u8>,
    buffer: BytesMut,
}

#[derive(Debug, Clone, Copy)]
pub struct OnsetSettings {
    white_led: bool,
    drum_decay_rate: f32,
    note_decay_rate: f32,
    hihat_decay: Duration,
    brightness: f32,
    timeout: u8,
}

impl Default for OnsetSettings {
    fn default() -> Self {
        Self {
            white_led: true,
            drum_decay_rate: 2.0,
            note_decay_rate: 4.0,
            hihat_decay: Duration::from_millis(200),
            brightness: 1.0,
            timeout: 2,
        }
    }
}

impl OnsetState {
    pub fn init(led_count: u16, rgbw: bool, brightness: f32, timeout: u8) -> Self {
        let prefix = if rgbw {
            vec![0x03, timeout]
        } else {
            vec![0x02, timeout]
        };
        let channels = 3 + usize::from(rgbw);
        let buffer = BytesMut::with_capacity(prefix.len() + led_count as usize * channels);
        OnsetState {
            led_count,
            rgbw,
            drum_envelope: DynamicDecay::init(2.0),
            note_envelope: DynamicDecay::init(4.0),
            hihat_envelope: FixedDecay::init(Duration::from_millis(200)),
            prefix,
            brightness,
            buffer,
        }
    }
}

impl Pollable for OnsetState {
    fn poll(&self) -> Bytes {
        let mut bytes = self.buffer.clone();
        bytes.clear();

        bytes.put_slice(&self.prefix);

        let red = self.drum_envelope.get_value() as f32 * self.led_count as f32 * 0.5;
        let blue = self.note_envelope.get_value() as f32 * self.led_count as f32 * 0.5;
        let white = self.hihat_envelope.get_value() as f32 * self.led_count as f32 * 0.2;

        let mut colors: Vec<Vec<u8>> = if self.rgbw {
            vec![vec![0, 0, 0, 0]; self.led_count as usize / 2]
        } else {
            vec![vec![0, 0, 0]; self.led_count as usize / 2]
        };

        for (i, color) in &mut colors.iter_mut().enumerate() {
            let r =
                ((red - i as f32).clamp(0.0, 1.0) * u8::MAX as f32 * self.brightness).round() as u8;
            let b = ((blue - i as f32).clamp(0.0, 1.0) * u8::MAX as f32 * self.brightness).round()
                as u8;
            let w = ((white - (self.led_count / 2 - i as u16) as f32).clamp(0.0, 1.0)
                * u8::MAX as f32
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
        Self::connect_with_settings(ip, OnsetSettings::default()).await
    }

    pub async fn connect_with_settings(ip: &str, settings: OnsetSettings) -> Result<LEDStripOnset, Box<dyn std::error::Error>> {
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
            .timeout(Duration::from_secs(settings.timeout as u64))
            .build()?;
        let url = format!("http://{}/json/info", ip);
        let resp = client.get(&url).send().await?;
        let info: Info = resp.json().await?;
        println!("Found strip {}", info.name);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let state = OnsetState::init(info.leds.count, info.leds.rgbw && settings.white_led, 1.0, settings.timeout);

        let state = Arc::new(Mutex::new(state));

        let polling_helper = PollingHelper::init(socket, state.clone(), 30.0);

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

impl LightService for LEDStripOnset {
    fn process_onset(&mut self, event: Onset) {
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
}

pub struct LEDStripSpectrum {
    strip: LEDStrip,
    polling_helper: PollingHelper,
    state: Arc<Mutex<SpectrumState>>,
}

#[derive(Debug, Clone, Copy)]
pub struct SpectrumSettings {
    pub leds_per_second: f64,
    pub center: bool,
    pub master_brightness: f32,
    pub min_brightness: f32,
    pub low_end_crossover: f32,
    pub high_end_crossover: f32,
    pub polling_rate: f64,
    pub timeout: u8,
}

impl Default for SpectrumSettings {
    fn default() -> Self {
        Self {
            leds_per_second: 100.0,
            center: true,
            master_brightness: 1.2,
            min_brightness: 0.25,
            low_end_crossover: 240.0,
            high_end_crossover: 2400.0,
            polling_rate: 50.0,
            timeout: 2,
        }
    }
}

impl LEDStripSpectrum {
    pub async fn connect(
        ip: &str,
        sampling_rate: f32,
    ) -> Result<LEDStripSpectrum, Box<dyn std::error::Error>> {
        Self::connect_with_settings(ip, sampling_rate, SpectrumSettings::default()).await
    }

    pub async fn connect_with_settings(
        ip: &str,
        sampling_rate: f32,
        settings: SpectrumSettings,
    ) -> Result<LEDStripSpectrum, Box<dyn std::error::Error>> {
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
            .timeout(Duration::from_secs(settings.timeout as u64))
            .build()?;
        let url = format!("http://{}/json/info", ip);
        let resp = client.get(&url).send().await?;
        let info: Info = resp.json().await?;
        println!("Found strip {}", info.name);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let samples_per_led = (sampling_rate as f64 / settings.leds_per_second).round() as u32;

        let state = SpectrumState::init(
            sampling_rate,
            info.leds.count,
            settings.master_brightness,
            settings.min_brightness,
            samples_per_led,
            settings.center,
            settings.timeout,
        );

        let state = Arc::new(Mutex::new(state));

        let polling_helper = PollingHelper::init(socket, state.clone(), 50.0);

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
}

impl LightService for LEDStripSpectrum {
    fn process_samples(&mut self, samples: &[f32]) {
        let mut state = self.state.lock().unwrap();
        state.visualize_spectrum(samples);
    }

    fn process_onset(&mut self, event: Onset) {
        let mut state = self.state.lock().unwrap();
        if let Onset::Full(strength) = event {
            state.envelope.trigger(strength)
        }
    }
}

pub struct SpectrumState {
    sample_buffer: VecDeque<f32>,
    colors: VecDeque<[u8; 3]>,
    prefix: Vec<u8>,
    led_count: u16,
    center: bool,
    master_brightness: f32,
    min_brightness: f32,
    samples_per_led: u32,
    low_pass_filter: DirectForm2Transposed<f32>,
    high_pass_filter: DirectForm2Transposed<f32>,
    envelope: DynamicDecay,
    buffer: BytesMut,
}

impl SpectrumState {
    pub fn init(
        sampling_frequency: f32,
        led_count: u16,
        master_brightness: f32,
        min_brightness: f32,
        samples_per_led: u32,
        center: bool,
        timeout: u8,
    ) -> Self {
        let prefix = vec![0x02, timeout];
        let low_pass = DirectForm2Transposed::<f32>::new(
            Coefficients::<f32>::from_params(
                Type::LowPass,
                sampling_frequency.hz(),
                240.hz(),
                Q_BUTTERWORTH_F32,
            )
            .unwrap(),
        );
        let high_pass = DirectForm2Transposed::<f32>::new(
            Coefficients::<f32>::from_params(
                Type::HighPass,
                sampling_frequency.hz(),
                2.4.khz(),
                Q_BUTTERWORTH_F32,
            )
            .unwrap(),
        );
        let bytes = BytesMut::with_capacity(prefix.len() + led_count as usize * 3);
        Self {
            sample_buffer: VecDeque::new(),
            colors: VecDeque::from(vec![[0, 0, 0]; led_count as usize]),
            prefix,
            led_count,
            center,
            master_brightness,
            min_brightness,
            samples_per_led,
            low_pass_filter: low_pass,
            high_pass_filter: high_pass,
            envelope: DynamicDecay::init(8.0),
            buffer: bytes,
        }
    }

    pub fn visualize_spectrum(&mut self, samples: &[f32]) {
        self.sample_buffer.extend(samples);
        let n = self.sample_buffer.len() / self.samples_per_led as usize;
        self.sample_buffer.make_contiguous();
        for _ in 0..n {
            let samples = self.sample_buffer.as_slices().0;

            let (low_weight, mid_weight, highs_weight) = samples
                .iter()
                .map(|s| {
                    (
                        self.low_pass_filter.run(*s),
                        *s,
                        self.high_pass_filter.run(*s),
                    )
                })
                .map(|(low, s, high)| (low, (s - low - high), high))
                .map(|(low, mid, high)| (low * low, mid * mid, high * high))
                .fold((0.0_f32, 0.0_f32, 0.0_f32), |acc, (low, mid, high)| {
                    (acc.0 + low, acc.1 + mid, acc.2 + high)
                });

            let (low_weight, mid_weight, highs_weight) = (
                (low_weight / self.samples_per_led as f32).sqrt(),
                (mid_weight / self.samples_per_led as f32).sqrt(),
                (highs_weight / self.samples_per_led as f32).sqrt(),
            );

            let max = low_weight.max(mid_weight.max(highs_weight));

            let brightness = ((self.envelope.get_value() * (1.0 - self.min_brightness))
                + self.min_brightness)
                * self.master_brightness; // Set a minimum quarter brightness

            let rgb = [
                (low_weight / max * 255.0 * brightness) as u8,
                (mid_weight / max * 255.0 * brightness) as u8,
                (highs_weight / max * 255.0 * brightness) as u8,
            ];

            let rgb = color_upsample(rgb);
            let [h, _, v] = rgb_to_hsv(rgb);
            let rgb = hsv_to_rgb(&[h, 1.0, v]);
            let rgb = color_downsample(rgb);

            self.colors.pop_front();
            self.colors.push_back(rgb);

            self.sample_buffer.drain(0..self.samples_per_led as usize);
        }
    }
}

impl Pollable for SpectrumState {
    fn poll(&self) -> Bytes {
        let mut bytes = self.buffer.clone();
        bytes.clear();
        bytes.put_slice(&self.prefix);

        if !self.center {
            for color in self.colors.iter().rev() {
                bytes.put_slice(color);
            }
        } else {
            for color in self
                .colors
                .iter()
                .rev()
                .take((self.led_count / 2 + self.led_count % 2) as usize)
                .rev()
                .chain(
                    self.colors
                        .iter()
                        .rev()
                        .skip((self.led_count % 2) as usize)
                        .take((self.led_count / 2) as usize),
                )
            {
                bytes.put_slice(color);
            }
        }

        bytes.into()
    }
}
