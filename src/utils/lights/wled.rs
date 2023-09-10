use std::{sync::{Mutex, Arc}, time::Duration};

use bytes::{Bytes, BytesMut, BufMut};
use serde::{Serialize, Deserialize};
use tokio::net::UdpSocket;

use super::{PollingHelper, envelope::{DynamicDecay, Envelope, FixedDecay}, Pollable, LightService, Event};

#[allow(dead_code)]
#[derive(Debug)]
pub struct LEDStrip {
    name: String,
    led_count: u16,
    polling_helper: PollingHelper,
    ip: String,
    port: u16,
    segments: Vec<Segment>,
    rgbw: bool,
    state: Arc<Mutex<State>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct Segment {
    start: usize,
    stop: usize,
}

#[derive(Debug)]
struct State {
    led_count: u16,
    brightness: f64,
    rgbw: bool,
    drum_envelope: DynamicDecay,
    note_envelope: DynamicDecay,
    hihat_envelope: FixedDecay,
    prefix: Vec<u8>
}

impl State {
    pub fn init(led_count: u16, rgbw: bool, brightness: f64) -> Self {
        let prefix = if rgbw {
            vec![0x03, 0x01]
        } else {
            vec![0x02, 0x01]
        };
        State { 
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

impl Pollable for State {
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
            let r = ((red - i as f64).clamp(0.0, 1.0) * u8::MAX as f64 * self.brightness).round() as u8;
            let b = ((blue - i as f64).clamp(0.0, 1.0) * u8::MAX as f64 * self.brightness).round() as u8;
            let w = ((white - (self.led_count / 2 - i as u16) as f64).clamp(0.0, 1.0) * u8::MAX as f64 * self.brightness).round() as u8;

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

impl LEDStrip {
    pub async fn connect(ip: &str) -> Result<LEDStrip, Box<dyn std::error::Error>> {
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
        let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build()?;
        let url = format!("http://{}/json/info", ip);
        let resp = client.get(&url).send().await?;
        let info: Info = resp.json().await?;
        println!("Found strip {}", info.name);

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let state = State::init(info.leds.count, info.leds.rgbw, 1.0);

        let state = Arc::new(Mutex::new(state));

        let polling_helper = PollingHelper::init(socket, state.clone(), 30);

        Ok(LEDStrip {
            name: info.name,
            led_count: info.leds.count,
            polling_helper,
            ip: ip.to_string(),
            port: info.udpport,
            segments: vec![Segment {
                start: 0,
                stop: info.leds.count as usize,
            }],
            rgbw: info.leds.rgbw,
            state,
        })
    }
}

impl LightService for LEDStrip {
    fn event_detected(&mut self, event: Event) {
        let mut state = self.state.lock().unwrap();
        match event {
            Event::Drum(strength) => {
                state.drum_envelope.trigger(strength);
            }
            Event::Hihat(strength) => {
                state.hihat_envelope.trigger(strength);
            }
            Event::Note(strength, _) => {
                state.note_envelope.trigger(strength);
            }
            _ => {}
        };
    }

    fn update(&mut self) {
        // self updating
    }
}
