use bytes::{BufMut, Bytes, BytesMut};
use ciborium::{from_reader, into_writer};
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
use tracing::{debug, info, warn};
use webrtc_dtls::{cipher_suite::CipherSuiteId, config::Config, conn::DTLSConn};

use super::{
    envelope::{self, Envelope},
    Closeable, Pollable, PollingHelper, Stream, Writeable,
};
use crate::utils::{audioprocessing::Onset, lights::LightService};

#[derive(Debug)]
pub enum HueError {
    Http(reqwest::Error),
    Handshake(webrtc_dtls::Error),
    VersionError(u32),
    TimeOut,
    NoBridgeFound,
    SaveBridgeError(std::io::Error),
    EntertainmentAreaNotFound,
    IPError(std::net::AddrParseError),
}

impl std::error::Error for HueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HueError::Http(e) => Some(e),
            HueError::Handshake(e) => Some(e),
            HueError::SaveBridgeError(e) => Some(e),
            HueError::IPError(e) => Some(e),
            _ => None,
        }
    }
}

impl Display for HueError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Http(_) => write!(f, "Http request failed"),
            Self::Handshake(_) => write!(f, "Dtls Handshake failed"),
            Self::VersionError(version) => write!(
                f,
                "Software version too low: {version}\nMust be at least 1948086000"
            ),
            Self::TimeOut => write!(f, "Timed out"),
            Self::NoBridgeFound => write!(f, "No Bridges could be found"),
            Self::SaveBridgeError(_) => write!(f, "Error saving bridges to file"),
            Self::EntertainmentAreaNotFound => write!(f, "Entertainment area could not be found"),
            Self::IPError(_) => write!(f, "IP address is in the wrong format"),
        }
    }
}

impl From<reqwest::Error> for HueError {
    fn from(err: reqwest::Error) -> Self {
        HueError::Http(err)
    }
}

impl From<webrtc_dtls::Error> for HueError {
    fn from(err: webrtc_dtls::Error) -> Self {
        HueError::Handshake(err)
    }
}

impl From<ciborium::ser::Error<std::io::Error>> for HueError {
    fn from(err: ciborium::ser::Error<std::io::Error>) -> Self {
        match err {
            ciborium::ser::Error::Io(err) => HueError::SaveBridgeError(err),
            ciborium::ser::Error::Value(_) => {
                panic!("Serialization failed, this should be impossible, please report")
            }
        }
    }
}

impl From<std::io::Error> for HueError {
    fn from(err: std::io::Error) -> Self {
        HueError::SaveBridgeError(err)
    }
}

impl From<std::net::AddrParseError> for HueError {
    fn from(value: std::net::AddrParseError) -> Self {
        HueError::IPError(value)
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

#[derive(Debug, Deserialize, Clone)]
struct UnauthenticatedBridge {
    #[serde(rename = "id")]
    _id: String,
    #[serde(rename = "internalipaddress")]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct HueSettings {
    #[serde(rename = "ip")]
    pub ip: Option<Ipv4Addr>,
    #[serde(rename = "area")]
    pub area: Option<String>,
    #[serde(rename = "auth_file")]
    pub auth_file: Option<String>,
    #[serde(flatten)]
    pub light_settings: LightSettings,
    pub push_link_timeout: Duration,
    pub timeout: Duration,
}

impl Default for HueSettings {
    fn default() -> Self {
        Self {
            ip: None,
            area: None,
            auth_file: None,
            light_settings: Default::default(),
            push_link_timeout: Duration::from_secs(30),
            timeout: Duration::from_secs(2),
        }
    }
}

impl BridgeManager {
    fn new(timeout: Duration) -> Self {
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .timeout(timeout)
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

    async fn filter_reachable(&self, bridges: &[BridgeData]) -> Vec<BridgeData> {
        let mut candidates: Vec<BridgeData> = Vec::new();
        for bridge in bridges {
            if self.check_bridge_reachable(&bridge.ip).await {
                candidates.push(bridge.clone());
            }
        }

        candidates
    }

    async fn check_bridge_reachable(&self, ip: &Ipv4Addr) -> bool {
        let Ok(config) = self.get_bridge_config(*ip).await else {
            return false;
        };

        info!("Found Bridge {}", &config.name);

        true
    }

    async fn search_bridges(&self) -> Result<Vec<UnauthenticatedBridge>, HueError> {
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

        for bridge in local_bridges.into_iter() {
            if !self
                .check_bridge_reachable(&bridge.ip_address.parse().unwrap())
                .await
            {
                continue;
            }

            bridges.push(UnauthenticatedBridge {
                _id: bridge.id,
                ip: bridge.ip_address.parse().unwrap(),
            });
        }
        Ok(bridges)
    }

    async fn locate_bridge(
        &self,
        ip: Option<Ipv4Addr>,
        timeout: Option<Duration>,
        save_file: &str,
    ) -> Result<BridgeData, HueError> {
        let mut saved_bridges = BridgeManager::load_saved_bridges(save_file);
        let mut found_bridges = self.filter_reachable(&saved_bridges).await;

        if let Some(ip) = ip {
            found_bridges.retain(|bridge| bridge.ip == ip);
        } else if found_bridges.len() > 1 {
            warn!("Multiple bridges found");
            for bridge in found_bridges.iter().rev() {
                let config = self.get_bridge_config(bridge.ip).await?;
                warn!("Name: {}, IP: {}", config.name, bridge.ip);
            }
            warn!("The first bridge will be selected");
            warn!("If you want to use a different bridge, please specify it with the given IP");
        }

        if !found_bridges.is_empty() {
            return Ok(found_bridges.pop().unwrap());
        }

        let mut new_bridges = self.search_bridges().await?;
        if let Some(ip) = ip {
            new_bridges.retain(|bridge| bridge.ip == ip);
        } else if new_bridges.len() > 1 {
            warn!("Multiple bridges found");
            for bridge in new_bridges.iter().rev() {
                let config = self.get_bridge_config(bridge.ip).await?;
                warn!("Name: {}, IP: {}", config.name, bridge.ip);
            }
            warn!("The first bridge will be selected");
            warn!("If you want to use a different bridge, please specify it with the given IP");
        }

        let bridge = new_bridges.pop().ok_or(HueError::NoBridgeFound)?;

        let bridge = self.authenticate_bridge(bridge.ip, timeout).await?;

        saved_bridges.push(bridge.clone());

        BridgeManager::save_bridges(&saved_bridges, save_file)?;

        Ok(bridge)
    }

    async fn authenticate_bridge(
        &self,
        ip: Ipv4Addr,
        timeout: Option<Duration>,
    ) -> Result<BridgeData, HueError> {
        #[derive(Serialize, Debug)]
        struct Body {
            devicetype: String,
            generateclientkey: bool,
        }
        let timeout = timeout.unwrap_or(HueSettings::default().push_link_timeout);
        let config = self.get_bridge_config(ip).await?;

        if config.swversion.parse::<u32>().unwrap() < 1948086000 {
            return Err(HueError::VersionError(
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

        warn!("Please press push link button");

        let mut saved_bridge = BridgeData {
            id: config.id,
            ip,
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
            _ = tokio::time::sleep(timeout) => {
                return Err(HueError::TimeOut);
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
            None => return Err(HueError::TimeOut),
        }

        info!("Authenticated with {}", config.name);

        Ok(saved_bridge)
    }

    fn save_bridges(bridges: &[BridgeData], path: &str) -> Result<(), HueError> {
        let f = File::create(path)?;
        into_writer(&bridges, f)?;
        info!("Saved authenticated bridges to {path}");
        Ok(())
    }

    async fn get_entertainment_areas(
        &self,
        bridge: &BridgeData,
    ) -> Result<Vec<EntertainmentArea>, HueError> {
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

    async fn get_bridge_config(&self, ip: Ipv4Addr) -> Result<BridgeConfig, HueError> {
        let response = self
            .client
            .get(format!("https://{}/api/0/config", ip))
            .send()
            .await?;

        Ok(response.json::<BridgeConfig>().await?)
    }
    async fn start_connection(
        &self,
        bridge: BridgeData,
        area: Option<String>,
    ) -> Result<BridgeConnection, HueError> {
        let settings = LightSettings::default();

        self.start_connection_with_settings(bridge, area, settings)
            .await
    }

    async fn start_connection_with_settings(
        &self,
        bridge: BridgeData,
        area: Option<String>,
        settings: LightSettings,
    ) -> Result<BridgeConnection, HueError> {
        let mut areas = self.get_entertainment_areas(&bridge).await?;

        if let Some(area) = area {
            areas.retain(|ent_area| ent_area.id == area);
        } else if areas.len() > 1 {
            warn!("Multiple areas found");
            for area in areas.iter().rev() {
                warn!("Name: {}, ID: {}", area._metadata._name, area.id);
            }
            warn!("The first area will be selected");
            warn!("If you want to use a different area, please specify it with the given ID");
        }
        let area = areas.pop().ok_or(HueError::EntertainmentAreaNotFound)?;

        BridgeConnection::with_settings(bridge, area, settings).await
    }
}

pub async fn connect() -> Result<BridgeConnection, HueError> {
    let manager = BridgeManager::new(HueSettings::default().timeout);

    let bridge = manager.locate_bridge(None, None, CONFIG_PATH).await?;

    manager.start_connection(bridge, None).await
}

pub async fn connect_by_ip(ip: Ipv4Addr) -> Result<BridgeConnection, HueError> {
    let manager = BridgeManager::new(HueSettings::default().timeout);

    let bridge = manager.locate_bridge(Some(ip), None, CONFIG_PATH).await?;

    manager.start_connection(bridge, None).await
}

pub async fn connect_with_settings(settings: HueSettings) -> Result<BridgeConnection, HueError> {
    let manager = BridgeManager::new(settings.timeout);

    let bridge = manager
        .locate_bridge(
            settings.ip,
            Some(settings.push_link_timeout),
            &settings.auth_file.unwrap_or(CONFIG_PATH.to_owned()),
        )
        .await?;

    manager
        .start_connection_with_settings(bridge, settings.area, settings.light_settings)
        .await
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
    async fn init(bridge: BridgeData, area: EntertainmentArea) -> Result<Self, HueError> {
        let settings = LightSettings::default();
        Self::with_settings(bridge, area, settings).await
    }

    async fn with_settings(
        bridge: BridgeData,
        area: EntertainmentArea,
        settings: LightSettings,
    ) -> Result<Self, HueError> {
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

        let state = Arc::new(Mutex::new(State::with_settings(&area, settings)));

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
    ) -> Result<reqwest::Response, HueError> {
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
    ) -> Result<DTLSConn, HueError> {
        let config = Config {
            cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
            psk: Some(Arc::new(move |_| Ok(decode_hex(psk.as_str()).unwrap()))),
            psk_identity_hint: Some(identity.to_vec()),
            server_name: "localhost".to_owned(),
            ..Default::default()
        };

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
            Onset::Kick(volume) => {
                if volume > state.drum.get_value() {
                    state.drum.trigger(volume);
                }
            }
            Onset::Hihat(volume) => {
                if volume > state.hihat.get_value() {
                    state.hihat.trigger(volume);
                }
            }
            Onset::Snare(volume) => {
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
    drum: envelope::DynamicDecay,
    hihat: envelope::FixedDecay,
    note: envelope::FixedDecay,
    fullband: envelope::Color,
    prefix: Vec<u8>,
    channels: Vec<u8>,
    color_envelope: bool,
    buffer: BytesMut,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub struct LightSettings {
    pub drum_decay_rate: f32,
    #[serde(rename = "NoteDecay")]
    pub note_decay: Duration,
    #[serde(rename = "HihatDecay")]
    pub hihat_decay: Duration,
    #[serde(rename = "FullbandDecay")]
    pub fullband_decay: Duration,
    pub fullband_color: ([u16; 3], [u16; 3]),
    pub color_envelope: bool,
}

impl Default for LightSettings {
    fn default() -> Self {
        Self {
            drum_decay_rate: 8.0,
            note_decay: Duration::from_millis(100),
            hihat_decay: Duration::from_millis(80),
            fullband_decay: Duration::from_millis(250),
            fullband_color: ([u16::MAX, 0, 0], [2, 0, 1]),
            color_envelope: false,
        }
    }
}

impl State {
    fn init(area: &EntertainmentArea) -> Self {
        Self::with_settings(area, LightSettings::default())
    }

    fn with_settings(area: &EntertainmentArea, settings: LightSettings) -> Self {
        let mut prefix = BytesMut::from("HueStream");
        prefix.extend([2, 0, 0, 0, 0, 0, 0]); // Api Version, empty sequence id, color space = RGB and reserved bytes. See also https://developers.meethue.com/develop/hue-entertainment/hue-entertainment-api/#getting-started-with-streaming-api
        prefix.put(area.id.as_bytes());

        let channels: Vec<_> = area.channels.iter().map(|chan| chan.channel_id).collect();
        let buffer_size = prefix.len() + 7 * channels.clone().len();
        State {
            drum: envelope::DynamicDecay::init(settings.drum_decay_rate),
            hihat: envelope::FixedDecay::init(settings.hihat_decay),
            note: envelope::FixedDecay::init(settings.note_decay),
            fullband: envelope::Color::init(
                settings.fullband_color.0,
                settings.fullband_color.1,
                settings.fullband_decay,
            ),
            prefix: prefix.into(),
            channels,
            color_envelope: settings.color_envelope,
            buffer: BytesMut::with_capacity(buffer_size),
        }
    }
}

impl Pollable for State {
    fn poll(&self) -> Bytes {
        let mut bytes = self.buffer.clone();
        bytes.clear();
        bytes.extend(self.prefix.clone());
        if self.color_envelope {
            for id in self.channels.iter() {
                bytes.put_u8(*id);
                let color = self.fullband.get_color();
                bytes.put_u16(color[0]);
                bytes.put_u16(color[1]);
                bytes.put_u16(color[2]);
            }
        } else {
            let r = (self.drum.get_value() * u16::MAX as f32) as u16;
            let white = (self.hihat.get_value() * u16::MAX as f32) as u16 >> 3;
            let b = (self.note.get_value() * u16::MAX as f32) as u16 >> 1;
            for id in self.channels.iter() {
                bytes.put_u8(*id);
                bytes.put_u16(r.saturating_add(white));
                bytes.put_u16(white);
                bytes.put_u16(b.saturating_add(white));
            }
        }

        bytes.into()
    }
}
