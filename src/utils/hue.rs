use futures::executor::block_on;
use log::info;
use std::{net::{SocketAddr, Ipv4Addr, IpAddr}, num::ParseIntError, sync::Arc, time::Duration};
use tokio::net::UdpSocket;
use webrtc_dtls::{conn::DTLSConn, config::Config, cipher_suite::CipherSuiteId, Error};

use super::lights::{PollingHelper, LightService, FixedDecayEnvelope};
use crate::utils::lights::{Event, MultibandEnvelope, Envelope, DynamicDecayEnvelope, ColorEnvelope};
#[allow(dead_code)]
pub struct Bridge {
    ip: Ipv4Addr,
    app_key: String,
    area: String,
    polling_helper: PollingHelper<Arc<DTLSConn>>,
    envelopes: MultibandEnvelope,
}

#[derive(Debug)]
pub enum ConnectionError {
    Mode(reqwest::Error),
    Handshake(Error)
}

impl Bridge {
    pub fn init() -> Result<Bridge, ConnectionError>{        
        let app_key = "q22b7aOctHn5xMecCBFpuIyYSdpS5rRMmTqXrQ9h";
        let app_id = "30e32a72-c564-4b8f-9897-26170d9aeb49";
        let area_id = "5fb0617b-4883-4a1b-86c4-c67b63a9d784";
        let psk = "3AD5F8F3F15A4BC195F774724F188334";
        let bridge_ip = "192.168.2.20".parse().unwrap();

        info!("Start Entertainment mode");
        match block_on(start_entertainment_mode(&bridge_ip, area_id, app_key)) {
            Ok(_) => {},
            Err(e) => {
                return Err(ConnectionError::Mode(e));
            }
        }
        info!("Building DTLS Connection");
        let connection = match block_on(dtls_connection(app_id.as_bytes().to_vec(), psk.to_owned(), IpAddr::V4(bridge_ip), 2100)) {
            Ok(conn) => conn,
            Err(e) => return Err(ConnectionError::Handshake(e))
        };
        info!("Connection established");

        let polling_helper = PollingHelper::init::<Arc<fn (&[[u16; 3]]) -> Vec<u8>>>(
            Arc::new(connection), 
            Arc::new(move |colors| {
                let mut bytes = "HueStream".as_bytes().to_vec(); // Prefix
                bytes.extend([2, 0, 0, 0, 0, 0, 0]);
                bytes.extend(area_id.as_bytes());  // area uuid
                (0..usize::min(7, colors.len()))
                .for_each(|i| {
                    let rgb = colors[i];
                    bytes.push(i as u8);
                    bytes.extend(rgb[0].to_be_bytes());
                    bytes.extend(rgb[1].to_be_bytes());
                    bytes.extend(rgb[2].to_be_bytes());
                });
                bytes
            })
            , 55
        );

        let envelopes = MultibandEnvelope {
            drum: DynamicDecayEnvelope::init(8.0),
            hihat: FixedDecayEnvelope::init(Duration::from_millis(80)),
            note: FixedDecayEnvelope::init(Duration::from_millis(100)),
            fullband: ColorEnvelope::init(&[u16::MAX, 0, 0], &[2, 0, 1], Duration::from_millis(250)),
        };

        let bridge = Bridge {ip: bridge_ip, app_key: app_key.to_string(), area: area_id.to_string(), polling_helper, envelopes};
        
        return Ok(bridge);
    }
}

impl LightService for Bridge {
    fn event_detected(&mut self, event: super::lights::Event) {
        match event {
            Event::Full(volume) => {
                if volume > self.envelopes.fullband.envelope.get_value() {
                    self.envelopes.fullband.trigger(volume)
                }
            },
            Event::Atmosphere(_, _volume) => {
                // let brightness = (volume * u16::MAX as f32) as u16 >> 4;
                // self.polling_helper.update_color(&[[0, brightness, 0]], true);
            },
            Event::Drum(volume) => {
                if volume > self.envelopes.drum.get_value() {
                    self.envelopes.drum.trigger(volume);
                }
            },
            Event::Hihat(volume) => {
                if volume > self.envelopes.hihat.get_value() {
                    self.envelopes.hihat.trigger(volume);
                }
            },
            Event::Note(_, volume) => {
                if volume > self.envelopes.note.get_value() {
                    self.envelopes.note.trigger(volume);
                }
            },
        }
    }

    fn update(&mut self) {
        self.polling_helper.update_color(&vec![[0,0,0]; 1], false);

        /*
        if self.envelopes.fullband.envelope.get_value() > 0.0 {
            self.polling_helper.update_color(&[self.envelopes.fullband.get_color()], false)
        }
         */
        let brightness = (self.envelopes.drum.get_value() * u16::MAX as f32) as u16;
        self.polling_helper.update_color(&[[brightness, 0, 0]], true);

        let brightness = (self.envelopes.hihat.get_value() * u16::MAX as f32) as u16 >> 3;
        self.polling_helper.update_color(&[[brightness, brightness, brightness]], true);
        let brightness = (self.envelopes.note.get_value() * u16::MAX as f32) as u16 >> 1;
        self.polling_helper.update_color(&[[0, 0, brightness]], true);
    }
}

async fn start_entertainment_mode(bridge_ip: &Ipv4Addr, area_id: &str, app_key: &str) -> Result<reqwest::Response, reqwest::Error>{
    let client = reqwest::Client::builder().danger_accept_invalid_certs(true).timeout(Duration::from_secs(5)).build()?;
    let url = format!("https://{bridge_ip}/clip/v2/resource/entertainment_configuration/{area_id}");
    client.put(url)
        .header("hue-application-key", app_key)
        .body("{\"action\":\"start\"}")
        .send().await
}

async fn dtls_connection(identity: Vec<u8>, psk: String, dest_ip: IpAddr, dest_port: u16) -> Result<DTLSConn, Error>{
    let config = Config {
        cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
        psk: Some(Arc::new(move | _ | {
            Ok(decode_hex(psk.as_str()).unwrap())
        })),
        psk_identity_hint: Some(identity),
        ..Default::default()
    };

    info!("Binding Socket");
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
    socket.connect(SocketAddr::new(dest_ip, dest_port)).await.unwrap();
    info!("Bound: {}", socket.local_addr().unwrap());
    DTLSConn::new(socket, config, true, None).await
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}