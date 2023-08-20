use ciborium::{from_reader, into_writer};
use futures::executor::block_on;
use log::{info, warn};
use reqwest::ClientBuilder;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::ParseIntError,
    sync::Arc,
    time::Duration, fmt::{Display, Formatter, self}, fs::File,
};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use webrtc_dtls::{cipher_suite::CipherSuiteId, config::Config, conn::DTLSConn};

use super::lights::{FixedDecayEnvelope, LightService, PollingHelper};
use crate::utils::lights::{
    ColorEnvelope, DynamicDecayEnvelope, Envelope, Event, MultibandEnvelope,
};
#[allow(dead_code)]
pub struct BridgeConnection {
    id: String,
    ip: Ipv4Addr,
    app_key: String,
    app_id: String,
    area: String,
    polling_helper: PollingHelper<Arc<DTLSConn>>,
    envelopes: MultibandEnvelope,
}

#[derive(Debug)]
pub enum ConnectionError {
    Http(reqwest::Error),
    Handshake(webrtc_dtls::Error),
    VersionError(u32),
    TimeOut
}

impl std::error::Error for ConnectionError {}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "Http request failed: {e}"),
            Self::Handshake(e) => write!(f, "Dtls Handshake failed: {e}"),
            Self::VersionError(version) => write!(f, "Software version too low: {version}\nMust be at least 1948086000"),
            Self::TimeOut => write!(f, "Timed out"),
        }
    }
}

impl From<reqwest::Error> for ConnectionError {
    fn from(err: reqwest::Error) -> Self {
        ConnectionError::Http(err)
    }
}

impl From<webrtc_dtls::Error> for ConnectionError {
    fn from(err: webrtc_dtls::Error) -> Self {
        ConnectionError::Handshake(err)
    }
}

impl BridgeConnection {
    pub fn init(bridge: SavedBridge, area: String) -> Result<BridgeConnection, ConnectionError> {
        let SavedBridge { id, ip, app_key, app_id, psk } = bridge;

        info!("Start Entertainment mode");
        block_on(start_entertainment_mode(&ip, &area, &app_key))?;
        info!("Building DTLS Connection");
        let connection = block_on(dtls_connection(
            app_id.as_bytes().to_vec(),
            psk.to_owned(),
            IpAddr::V4(ip),
            2100,
        ))?;
        info!("Connection established");

        let area_id = area.clone(); 

        let polling_helper = PollingHelper::init::<Arc<fn(&[[u16; 3]]) -> Vec<u8>>>(
            Arc::new(connection),
            Arc::new(move |colors| {
                let mut bytes = "HueStream".as_bytes().to_vec(); // Prefix
                bytes.extend([2, 0, 0, 0, 0, 0, 0]);
                bytes.extend(area_id.as_bytes()); // area UUID
                (0..usize::min(7, colors.len())).for_each(|i| {
                    let rgb = colors[i];
                    bytes.push(i as u8);
                    bytes.extend(rgb[0].to_be_bytes());
                    bytes.extend(rgb[1].to_be_bytes());
                    bytes.extend(rgb[2].to_be_bytes());
                });
                bytes
            }),
            55,
        );

        let envelopes = MultibandEnvelope {
            drum: DynamicDecayEnvelope::init(8.0),
            hihat: FixedDecayEnvelope::init(Duration::from_millis(80)),
            note: FixedDecayEnvelope::init(Duration::from_millis(100)),
            fullband: ColorEnvelope::init(
                &[u16::MAX, 0, 0],
                &[2, 0, 1],
                Duration::from_millis(250),
            ),
        };

        let bridge = BridgeConnection {
            id,
            ip,
            app_key,
            app_id,
            area: area,
            polling_helper,
            envelopes,
        };

        return Ok(bridge);
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavedBridge {
    pub id: String,
    pub ip: Ipv4Addr,
    pub app_key: String,
    pub app_id: String,
    pub psk: String,
}

pub struct Bridge {
    id: String,
    ip: Ipv4Addr,
}

#[derive(Debug, Deserialize)]
enum ApiResponse {
    #[serde(rename = "success")]
    Success {username: String, clientkey: String},
    #[serde(rename = "error")]
    Error {description: String}
}

pub async fn connect() -> Result<BridgeConnection, ConnectionError> {
    let mut saved_bridges:Vec<SavedBridge> = Vec::new();
    let mut candidates:Vec<SavedBridge> = Vec::new();
    if let Ok(file) = File::open("hue.cbor") {
        let data: Vec<SavedBridge> = from_reader(file).unwrap();
        for bridge in data {
            saved_bridges.push(bridge.clone());
            if check_bridge(&bridge.ip).await {
                candidates.push(bridge);
            }
        }
    }
    if candidates.len() == 0 {
        let mut local_bridges = find_bridges().await?;
        local_bridges.retain(|b| saved_bridges.iter().map(|save| save.ip.to_string()).find(|ip| b.ip.to_string() == ip.to_owned()).is_none());
        for bridge in local_bridges {
            if let Ok(authenticated) = bridge.authenticate().await {
                saved_bridges.push(authenticated.clone());
                candidates.push(authenticated.clone());
            }
        }
    }

    if candidates.len() == 0 {
        warn!("Couldn't find compatible bridge");
        return Err(ConnectionError::TimeOut);
    }

    let f = File::create("hue.cbor").unwrap();
    into_writer(&saved_bridges, f).unwrap();

    #[derive(Deserialize, Debug)]
    struct _Metadata {
        #[serde(rename = "name")]
        _name: String
    }

    #[derive(Deserialize, Debug)]
    struct _EntertainmentArea {
        id: String,
        #[serde(rename = "metadata")]
        _metadata: _Metadata,
    }

    #[derive(Deserialize, Debug)]
    struct _EntResponse {
        data: Vec<_EntertainmentArea>
    }

    // TODO: Add ability to select bridge
    let bridge = candidates[0].clone();

    let client = ClientBuilder::new().danger_accept_invalid_certs(true).build().unwrap();

    let response = client
        .get(format!("https://{}/clip/v2/resource/entertainment_configuration", &bridge.ip))
        .header("hue-application-key", &bridge.app_key)
        .send()
        .await?;

    let response = response.json::<_EntResponse>().await?;

    // TODO: Allow selection of entertainment area
    let area = (&response.data[0].id).to_string();

    let bridge = BridgeConnection::init(bridge, area)?;

    Ok(bridge)
}

async fn check_bridge(ip: &Ipv4Addr) -> bool {
    let url = format!("http://{}/api/config", ip);
    let response = match reqwest::get(url).await {
        Ok(r) => r,
        Err(_) => return false,
    };

    #[derive(Deserialize, Debug)]
    struct BridgeConfig {
        name: String,
        swversion: String,
    }

    let config = response.json::<BridgeConfig>().await;

    if let Err(_) = config {
        return false;
    }

    let config = config.unwrap();

    println!("Found Bridge {}", &config.name);

    if config.swversion.parse::<u32>().unwrap() < 1948086000 {
        return false;
    }
    true

}

pub async fn find_bridges() -> Result<Vec<Bridge>, ConnectionError> {
    let response = reqwest::get("https://discovery.meethue.com/").await?.error_for_status()?;
    
    #[derive(Deserialize, Debug)]
    struct BridgeJson {
        id: String,
        #[serde(rename = "internalipaddress")]
        ip_address: String,
    }

    let local_bridges = response.json::<Vec<BridgeJson>>().await?;

    let mut bridges: Vec<Bridge> = Vec::new();

    for bridge in &local_bridges {
        if !check_bridge(&bridge.ip_address.parse().unwrap()).await {
            break;
        }

        bridges.push(
            Bridge { id: bridge.id.to_owned(), ip: bridge.ip_address.parse().unwrap() }
        );
    }
    Ok(bridges)
}

impl Bridge {
    pub async fn authenticate(&self) -> Result<SavedBridge, ConnectionError> {
        let response = reqwest::get(format!("http://{}/api/config", self.ip)).await?;

        #[derive(Deserialize)]
        struct BridgeConfig {
            #[serde(rename = "name")]
            _name: String,
            swversion: String,
        }
        let config = response.json::<BridgeConfig>().await?;

        if config.swversion.parse::<u32>().unwrap() < 1948086000 {
            return Err(ConnectionError::VersionError(config.swversion.parse::<u32>().unwrap()))
        }

        let client = reqwest::ClientBuilder::new().danger_accept_invalid_certs(true).build().unwrap();

        let hostname = gethostname::gethostname().into_string().unwrap().retain(|a| a != '\"');

        #[derive(Serialize, Debug)]
        struct Body {
            devicetype: String,
            generateclientkey: bool,
        }

        let devicetype = format!("music_sync#{hostname:?}");
        let params = Body {
            devicetype: devicetype,
            generateclientkey: true,
        };

        println!("Please press push link button");

        let mut timeout = 0;
        let mut saved_bridge = SavedBridge {
            id: self.id.to_string(),
            ip: self.ip,
            app_key: "".to_string(),
            app_id: "".to_string(),
            psk: "".to_string(),
            
        };
        loop {
            let response = client.post(format!("https://{}/api", self.ip))
            .json(&params)
            .send()
            .await?;

            match response.json::<Vec<ApiResponse>>().await {
                Ok(s) => {
                    match &s[0] {
                        ApiResponse::Success { username, clientkey } => {
                            saved_bridge.app_key = username.to_string();
                            saved_bridge.psk = clientkey.to_string();
                            break;
                        }
                        ApiResponse::Error { description } => {
                            warn!("Error: {description}");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            timeout += 1;
                            if timeout >=30 {
                                return Err(ConnectionError::TimeOut);
                            }
                        }
                    };
                },
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    timeout += 1;
                    if timeout >= 30 {
                        return Err(ConnectionError::TimeOut);
                    }
                },
            };
        }
        let response = client.get(format!("https://{}/auth/v1", self.ip)).header("hue-application-key", &saved_bridge.app_key).send().await?;
        match response.headers().get("hue-application-id") {
            Some(h) => {
                saved_bridge.app_id = h.to_str().unwrap().to_string()
            },
            None => return Err(ConnectionError::TimeOut),
        }

        Ok(saved_bridge)
    }
}

impl LightService for BridgeConnection {
    fn event_detected(&mut self, event: super::lights::Event) {
        match event {
            Event::Full(volume) => {
                if volume > self.envelopes.fullband.envelope.get_value() {
                    self.envelopes.fullband.trigger(volume)
                }
            }
            Event::Atmosphere(_, _volume) => {
                // let brightness = (volume * u16::MAX as f32) as u16 >> 4;
                // self.polling_helper.update_color(&[[0, brightness, 0]], true);
            }
            Event::Drum(volume) => {
                if volume > self.envelopes.drum.get_value() {
                    self.envelopes.drum.trigger(volume);
                }
            }
            Event::Hihat(volume) => {
                if volume > self.envelopes.hihat.get_value() {
                    self.envelopes.hihat.trigger(volume);
                }
            }
            Event::Note(volume, _) => {
                if volume > self.envelopes.note.get_value() {
                    self.envelopes.note.trigger(volume);
                }
            }
        }
    }

    fn update(&mut self) {
        self.polling_helper.update_color(&vec![[0, 0, 0]; 1], false);

        /*
        if self.envelopes.fullband.envelope.get_value() > 0.0 {
            self.polling_helper.update_color(&[self.envelopes.fullband.get_color()], false)
        }
         */
        let brightness = (self.envelopes.drum.get_value() * u16::MAX as f32) as u16;
        self.polling_helper
            .update_color(&[[brightness, 0, 0]], true);

        let brightness = (self.envelopes.hihat.get_value() * u16::MAX as f32) as u16 >> 3;
        self.polling_helper
            .update_color(&[[brightness, brightness, brightness]], true);
        let brightness = (self.envelopes.note.get_value() * u16::MAX as f32) as u16 >> 1;
        self.polling_helper
            .update_color(&[[0, 0, brightness]], true);
    }
}

async fn start_entertainment_mode(
    bridge_ip: &Ipv4Addr,
    area_id: &str,
    app_key: &str,
) -> Result<reqwest::Response, ConnectionError> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
        .build()?;
    let url = format!("https://{bridge_ip}/clip/v2/resource/entertainment_configuration/{area_id}");
    Ok(client
        .put(url)
        .header("hue-application-key", app_key)
        .body("{\"action\":\"start\"}")
        .send()
        .await?)
}

async fn dtls_connection(
    identity: Vec<u8>,
    psk: String,
    dest_ip: IpAddr,
    dest_port: u16,
) -> Result<DTLSConn, ConnectionError> {
    let config = Config {
        cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
        psk: Some(Arc::new(move |_| Ok(decode_hex(psk.as_str()).unwrap()))),
        psk_identity_hint: Some(identity),
        ..Default::default()
    };

    info!("Binding Socket");
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
    socket
        .connect(SocketAddr::new(dest_ip, dest_port))
        .await
        .unwrap();
    info!("Bound: {}", socket.local_addr().unwrap());
    Ok(DTLSConn::new(socket, config, true, None).await?)
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}
