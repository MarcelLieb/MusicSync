use std::sync::{Mutex, Arc};

use bytes::{Bytes, BytesMut, BufMut};
use serde::{Serialize, Deserialize};
use tokio::net::UdpSocket;

use super::{PollingHelper, envelope::{AnimationHelper, DynamicDecay, Envelope}, Pollable, LightService, Event};

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

struct Segment {
    start: usize,
    stop: usize,
}

struct State {
    led_count: u16,
    rgbw: bool,
    envelope: DynamicDecay
}

impl Pollable for State {
    fn poll(&self) -> Bytes {
        let mut data = BytesMut::with_capacity(2 + self.led_count as usize * 3);
        data.put_u8(0x03); // DRGBW Mode
        data.put_u8(0x01); // Timeout in seconds

        // Blink all LEDs
        for _ in 0..self.led_count {
            let value = (self.envelope.get_value() * u8::MAX as f32) as u8;
            let color = [0, 0, 0, value];
            data.put_slice(&color);
        }

        data.into()
    }
}

impl LEDStrip {
    pub async fn connect(ip: &str) -> Result<LEDStrip, Box<dyn std::error::Error>> {
        #[derive(Debug, Serialize, Deserialize)]
        struct leds {
            count: u16,
            rgbw: bool,
        }

        #[derive(Debug, Serialize, Deserialize)]
        struct Info {
            name: String,
            udpport: u16,
            leds: leds,
            ver: String,
        }

        let url = format!("http://{}/json/info", ip);
        let resp = reqwest::get(&url).await?;
        let info: Info = resp.json().await?;

        let mut animater = AnimationHelper::init(|pos| pos < 1000, 2000, true);
        let envelope = DynamicDecay::init(8.0);
        animater.start();

        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect((ip, info.udpport)).await?;

        let state = State {
            led_count: info.leds.count,
            rgbw: info.leds.rgbw,
            envelope,
        };

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
        match event {
            Event::Drum(strength) => {
                let mut state = self.state.lock().unwrap();
                state.envelope.trigger(strength);
            }
            _ => {}
        };
    }

    fn update(&mut self) {
        // self updating
    }
}
