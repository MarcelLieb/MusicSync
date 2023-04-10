use futures::executor;
use log::info;
use std::{net::{SocketAddr, Ipv4Addr, IpAddr}, num::ParseIntError, sync::Arc};
use tokio::net::UdpSocket;
use webrtc_dtls::{conn::DTLSConn, config::Config, cipher_suite::CipherSuiteId, Error};
#[allow(dead_code)]
pub struct Bridge {
    ip: Ipv4Addr,
    stream: DTLSConn,
    area: String
}

#[derive(Debug)]
pub enum ConnectionError {
    Mode(reqwest::Error),
    Handshake(Error)
}

impl Bridge {
    pub fn init() -> Result<Bridge, ConnectionError>{
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build().unwrap();
        
        let app_key = "q22b7aOctHn5xMecCBFpuIyYSdpS5rRMmTqXrQ9h";
        let app_id = "30e32a72-c564-4b8f-9897-26170d9aeb49";
        let area_id = "5fb0617b-4883-4a1b-86c4-c67b63a9d784";
        let psk = "3AD5F8F3F15A4BC195F774724F188334";
        let bridge_ip = "192.168.2.20".parse().unwrap();

        info!("Start Entertainment mode");
        match executor::block_on(start_entertainment_mode(&client, &bridge_ip, area_id, app_key)) {
            Ok(_) => {},
            Err(e) => {
                return Err(ConnectionError::Mode(e));
            }
        }
        info!("Building DTLS Connection");
        let connection = match executor::block_on(dtls_connection(app_id.as_bytes().to_vec(), psk.to_owned(), IpAddr::V4(bridge_ip), 2100)) {
            Ok(conn) => conn,
            Err(e) => return Err(ConnectionError::Handshake(e))
        };
        info!("Connection established");

        let bridge = Bridge {ip: bridge_ip, stream: connection, area: area_id.to_owned()};
        
        return Ok(bridge);
    }

    pub async fn send_color(&self, rgb: &[u8; 3]) {
        let mut bytes = "HueStream".as_bytes().to_vec(); // Prefix
        bytes.push(2);  // Major Version
        bytes.push(0);  // Minor Version
        bytes.push(0);  // Seq Id
        bytes.push(0);  // reserved
        bytes.push(0);  // reserved
        bytes.push(0);  // Color Space RGB
        bytes.push(0);  // reserved
        bytes.extend(self.area.as_bytes());  // area uuid
        bytes.push(0);  // channel number  
        bytes.push(rgb[0]);
        bytes.push(255);
        bytes.push(rgb[1]);
        bytes.push(255);
        bytes.push(rgb[2]);
        bytes.push(255);
        self.stream.write(&bytes, None).await.unwrap();
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        executor::block_on(self.stream.close()).unwrap();
    }
}

async fn start_entertainment_mode(client: &reqwest::Client, bridge_ip: &Ipv4Addr, area_id: &str, app_key: &str) -> Result<reqwest::Response, reqwest::Error>{
    let url = "https://".to_owned() + bridge_ip.to_string().as_str() + "/clip/v2/resource/entertainment_configuration/" + area_id;
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