use bytes::{BufMut, Bytes, BytesMut};
use ciborium::{from_reader, into_writer};
use log::{debug, error, info, warn};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    fs::File,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::ParseIntError,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::UdpSocket, select};
use webrtc_dtls::{cipher_suite::CipherSuiteId, config::Config, conn::DTLSConn};

use super::{envelope::Envelope, Closeable, Pollable, PollingHelper, Stream, Writeable};
use crate::utils::{
    audioprocessing::Onset,
    lights::{
        envelope::{Color, DynamicDecay, FixedDecay},
        LightService,
    },
};

#[derive(Debug)]
pub enum ConnectionError {
    Http(reqwest::Error),
    Handshake(webrtc_dtls::Error),
    VersionError(u32),
    TimeOut,
    NoBridgeFound,
    SaveBridgeError(std::io::Error),
    EntertainmentAreaNotFound,
}

impl std::error::Error for ConnectionError {}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Http(e) => write!(f, "Http request failed: {e}"),
            Self::Handshake(e) => write!(f, "Dtls Handshake failed: {e}"),
            Self::VersionError(version) => write!(
                f,
                "Software version too low: {version}\nMust be at least 1948086000"
            ),
            Self::TimeOut => write!(f, "Timed out"),
            Self::NoBridgeFound => write!(f, "No Bridges could be found"),
            Self::SaveBridgeError(e) => write!(f, "Error saving bridges to file: {e}"),
            Self::EntertainmentAreaNotFound => write!(f, "Entertainment area could not be found"),
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

impl From<ciborium::ser::Error<std::io::Error>> for ConnectionError {
    fn from(err: ciborium::ser::Error<std::io::Error>) -> Self {
        match err {
            ciborium::ser::Error::Io(err) => ConnectionError::SaveBridgeError(err),
            ciborium::ser::Error::Value(_) => {
                panic!("Serialization failed, this should be impossible, please report")
            }
        }
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(err: std::io::Error) -> Self {
        ConnectionError::SaveBridgeError(err)
    }
}

impl Writeable for DTLSConn {
    async fn write_data(&mut self, data: &Bytes) -> std::io::Result<()> {
        match self.write(data, None).await {
            Ok(_) => Ok(()),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("DTLS write failed: {e}"),
            )),
        }
    }
}

impl Closeable for DTLSConn {
    async fn close_connection(&mut self) {
        self.close().await.unwrap();
    }
}

impl Stream for DTLSConn {}

// TODO: Move save file to a proper permanent location
static CONFIG_PATH: &str = "hue.cbor";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeData {
    pub id: String,
    pub ip: Ipv4Addr,
    pub app_key: String,
    pub app_id: String,
    pub psk: String,
}

struct UnauthenticatedBridge {
    _id: String,
    ip: Ipv4Addr,
}

#[derive(Debug, Deserialize)]
enum ApiResponse {
    #[serde(rename = "success")]
    Success { username: String, clientkey: String },
    #[serde(rename = "error")]
    Error { description: String },
}

struct BridgeManager {
    client: Client,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
struct BridgeConfig {
    name: String,
    swversion: String,
    #[serde(rename = "bridgeid")]
    id: String,
}

#[derive(Deserialize, Debug, Clone)]
struct _Metadata {
    #[serde(rename = "name")]
    _name: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct EntertainmentChannels {
    channel_id: u8,
    #[serde(rename = "position")]
    _position: Point,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone, Copy)]
struct Point {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Deserialize, Debug, Clone)]
struct EntertainmentArea {
    id: String,
    #[serde(rename = "metadata")]
    _metadata: _Metadata,
    channels: Vec<EntertainmentChannels>,
}

impl BridgeManager {
    fn new() -> Self {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
        BridgeManager { client }
    }

    fn load_saved_bridges(path: &str) -> Vec<BridgeData> {
        let mut saved_bridges: Vec<BridgeData> = Vec::new();

        if let Ok(file) = File::open(path) {
            let data: Vec<BridgeData> = from_reader(file).unwrap();
            for bridge in data {
                saved_bridges.push(bridge.clone());
            }
        }

        saved_bridges
    }

    async fn check_bridges(&self, bridges: &[BridgeData]) -> Vec<BridgeData> {
        let mut candidates: Vec<BridgeData> = Vec::new();
        for bridge in bridges {
            if self.check_bridge(&bridge.ip).await {
                candidates.push(bridge.clone());
            }
        }

        candidates
    }

    async fn check_bridge(&self, ip: &Ipv4Addr) -> bool {
        let url = format!("http://{ip}/api/config");
        let Ok(response) = self.client.get(url).send().await else {
            return false;
        };

        let config = response.json::<BridgeConfig>().await;

        if config.is_err() {
            return false;
        }

        let config = config.unwrap();

        info!("Found Bridge {}", &config.name);

        !(config.swversion.parse::<u32>().unwrap() < 1948086000)
    }

    async fn search_bridges(&self) -> Result<Vec<UnauthenticatedBridge>, ConnectionError> {
        #[derive(Deserialize, Debug)]
        struct BridgeJson {
            id: String,
            #[serde(rename = "internalipaddress")]
            ip_address: String,
        }

        let response = self
            .client
            .get("https://discovery.meethue.com/")
            .send()
            .await?
            .error_for_status()?;

        let local_bridges = response.json::<Vec<BridgeJson>>().await?;

        let mut bridges: Vec<UnauthenticatedBridge> = Vec::new();

        for bridge in &local_bridges {
            if !self.check_bridge(&bridge.ip_address.parse().unwrap()).await {
                break;
            }

            bridges.push(UnauthenticatedBridge {
                _id: bridge.id.clone(),
                ip: bridge.ip_address.parse().unwrap(),
            });
        }
        Ok(bridges)
    }

    async fn authenticate_bridge(&self, ip: Ipv4Addr) -> Result<BridgeData, ConnectionError> {
        #[derive(Serialize, Debug)]
        struct Body {
            devicetype: String,
            generateclientkey: bool,
        }

        let response = self
            .client
            .get(format!("https://{}/api/0/config", ip))
            .send()
            .await?;

        let config = response.json::<BridgeConfig>().await?;

        if config.swversion.parse::<u32>().unwrap() < 1948086000 {
            return Err(ConnectionError::VersionError(
                config.swversion.parse::<u32>().unwrap(),
            ));
        }

        let mut hostname = gethostname::gethostname().into_string().unwrap();
        hostname.retain(|a| a != '\"');

        let devicetype = format!("music_sync#{hostname:?}");
        let params = Body {
            devicetype,
            generateclientkey: true,
        };

        println!("Please press push link button");

        let mut saved_bridge = BridgeData {
            id: config.id,
            ip: ip,
            app_key: String::new(),
            app_id: String::new(),
            psk: String::new(),
        };

        select! {
            _ = async {
                loop {
                    let response = self.client
                        .post(format!("https://{}/api", ip))
                        .json(&params)
                        .send()
                        .await?;

                    if let Ok(s) = response.json::<Vec<ApiResponse>>().await {
                        match &s[0] {
                            ApiResponse::Success {
                                username,
                                clientkey,
                            } => {
                                saved_bridge.app_key = username.to_string();
                                saved_bridge.psk = clientkey.to_string();
                                break;
                            }
                            ApiResponse::Error { description } => {
                                warn!("Error: {description}");
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        };
                    }
                }
                Ok::<_, reqwest::Error>(())
            } => {}
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                return Err(ConnectionError::TimeOut);
            }
        }

        let response = self
            .client
            .get(format!("https://{}/auth/v1", ip))
            .header("hue-application-key", &saved_bridge.app_key)
            .send()
            .await?;
        match response.headers().get("hue-application-id") {
            Some(h) => saved_bridge.app_id = h.to_str().unwrap().to_string(),
            None => return Err(ConnectionError::TimeOut),
        }

        Ok(saved_bridge)
    }

    fn save_bridges(bridges: &[BridgeData], path: &str) -> Result<(), ConnectionError> {
        let f = File::create(path)?;
        into_writer(&bridges, f)?;
        Ok(())
    }

    async fn get_entertainment_areas(
        &self,
        bridge: &BridgeData,
    ) -> Result<Vec<EntertainmentArea>, ConnectionError> {
        #[derive(Deserialize, Debug)]
        struct _EntResponse {
            data: Vec<EntertainmentArea>,
        }

        let response = self
            .client
            .get(format!(
                "https://{}/clip/v2/resource/entertainment_configuration",
                &bridge.ip
            ))
            .header("hue-application-key", &bridge.app_key)
            .send()
            .await?;

        let response = response.json::<_EntResponse>().await?;
        Ok(response.data)
    }
}

pub async fn connect() -> Result<BridgeConnection, ConnectionError> {
    let manager = BridgeManager::new();

    let authenticated_bridges = BridgeManager::load_saved_bridges(CONFIG_PATH);
    let found_bridges = manager.check_bridges(&authenticated_bridges).await;

    if found_bridges.is_empty() {
        let new_bridge = manager
            .search_bridges().await?.pop().ok_or_else(|| ConnectionError::NoBridgeFound)?;

        connect_to_bridge(new_bridge.ip).await
    }
    else {
        connect_to_bridge(found_bridges[0].ip).await
    }
}

pub async fn connect_to_bridge(ip: Ipv4Addr) -> Result<BridgeConnection, ConnectionError> {
    let manager = BridgeManager::new();

    let mut authenticated_bridges = BridgeManager::load_saved_bridges(CONFIG_PATH);

    let candidates: Vec<BridgeData> = authenticated_bridges
        .iter()
        .filter(|bridge| bridge.ip == ip)
        .map(|bridge| bridge.clone())
        .collect();

    let mut candidates = manager.check_bridges(&candidates).await;

    if candidates.is_empty() {
        let bridge = manager.authenticate_bridge(ip).await?;
        candidates.push(bridge.clone());
        authenticated_bridges.push(bridge.clone());
        BridgeManager::save_bridges(&authenticated_bridges, CONFIG_PATH)?;
    }

    if candidates.is_empty() {
        return Err(ConnectionError::NoBridgeFound);
    }
    let bridge = candidates.pop().unwrap();

    let areas = manager.get_entertainment_areas(&bridge).await?;


    if areas.is_empty() {
        error!(
            "No entertainment areas are configured\nPlease setup an entertainment area in the Philips Hue App"
        );
        return Err(ConnectionError::EntertainmentAreaNotFound);
    }
    
    if areas.len() > 1 {
        warn!("Multiple entertainment areas found:");
        for area in &areas {
            warn!("{} with id: {}", area._metadata._name, area.id);
        }
        warn!("The first one will be selected\nIf you want to connect to a different area, please specify it with the id");
    }

    let area = areas[0].clone();

    BridgeConnection::init(bridge, area).await
}

#[allow(dead_code)]
pub struct BridgeConnection {
    id: String,
    ip: Ipv4Addr,
    app_key: String,
    app_id: String,
    area: EntertainmentArea,
    polling_helper: PollingHelper,
    state: Arc<Mutex<State>>,
}

impl BridgeConnection {
    async fn init(
        bridge: BridgeData,
        area: EntertainmentArea,
    ) -> Result<BridgeConnection, ConnectionError> {
        let BridgeData {
            id,
            ip,
            app_key,
            app_id,
            psk,
        } = bridge;

        info!("Starting entertainment mode");
        Self::start_entertainment_mode(&ip, &area.id, &app_key).await?;

        info!("Building DTLS connection");
        let connection =
            Self::dtls_connection(app_id.as_bytes(), psk.clone(), IpAddr::V4(ip), 2100).await?;
        info!("Connection established");

        let state = Arc::new(Mutex::new(State::init(&area)));

        let polling_helper = PollingHelper::init(connection, state.clone(), 55.0);

        let bridge = BridgeConnection {
            id,
            ip,
            app_key,
            app_id,
            area,
            polling_helper,
            state,
        };
        Ok(bridge)
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
        let url =
            format!("https://{bridge_ip}/clip/v2/resource/entertainment_configuration/{area_id}");
        Ok(client
            .put(url)
            .header("hue-application-key", app_key)
            .body("{\"action\":\"start\"}")
            .send()
            .await?)
    }

    async fn dtls_connection(
        identity: &[u8],
        psk: String,
        dest_ip: IpAddr,
        dest_port: u16,
    ) -> Result<DTLSConn, ConnectionError> {
        let config = Config {
            cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
            psk: Some(Arc::new(move |_| Ok(decode_hex(psk.as_str()).unwrap()))),
            psk_identity_hint: Some(identity.to_vec()),
            ..Default::default()
        };

        info!("Binding Socket");
        let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await.unwrap());
        socket
            .connect(SocketAddr::new(dest_ip, dest_port))
            .await
            .unwrap();
        debug!("Bound: {}", socket.local_addr().unwrap());
        Ok(DTLSConn::new(socket, config, true, None).await?)
    }
}

impl LightService for BridgeConnection {
    fn process_onset(&mut self, event: Onset) {
        let mut state = self.state.lock().unwrap();
        match event {
            Onset::Full(volume) => {
                if volume > state.fullband.envelope.get_value() {
                    state.fullband.trigger(volume);
                }
            }
            Onset::Drum(volume) => {
                if volume > state.drum.get_value() {
                    state.drum.trigger(volume);
                }
            }
            Onset::Hihat(volume) => {
                if volume > state.hihat.get_value() {
                    state.hihat.trigger(volume);
                }
            }
            Onset::Note(volume, _) => {
                if volume > state.note.get_value() {
                    state.note.trigger(volume);
                }
            }
            _ => {}
        }
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}

struct State {
    drum: DynamicDecay,
    hihat: FixedDecay,
    note: FixedDecay,
    fullband: Color,
    prefix: Vec<u8>,
    channels: Vec<u8>,
}

impl State {
    fn init(area: &EntertainmentArea) -> State {
        let mut prefix = BytesMut::from("HueStream");
        prefix.extend([2, 0, 0, 0, 0, 0, 0]); // Api Version, empty sequence id, color space = RGB and reserved bytes. See also https://developers.meethue.com/develop/hue-entertainment/hue-entertainment-api/#getting-started-with-streaming-api
        prefix.put(area.id.as_bytes());

        State {
            drum: DynamicDecay::init(8.0),
            hihat: FixedDecay::init(Duration::from_millis(80)),
            note: FixedDecay::init(Duration::from_millis(100)),
            fullband: Color::init([u16::MAX, 0, 0], [2, 0, 1], Duration::from_millis(250)),
            prefix: prefix.into(),
            channels: area.channels.iter().map(|chan| chan.channel_id).collect(),
        }
    }
}

impl Pollable for State {
    fn poll(&self) -> Bytes {
        let r = (self.drum.get_value() * u16::MAX as f32) as u16;
        let white = (self.hihat.get_value() * u16::MAX as f32) as u16 >> 3;
        let b = (self.note.get_value() * u16::MAX as f32) as u16 >> 1;

        let mut bytes = BytesMut::with_capacity(self.prefix.len() + 7 * self.channels.len());
        bytes.extend(self.prefix.clone());
        for id in self.channels.iter() {
            bytes.put_u8(*id);
            bytes.put_u16(r.saturating_add(white));
            bytes.put_u16(white);
            bytes.put_u16(b.saturating_add(white));
        }

        bytes.into()
    }
}
