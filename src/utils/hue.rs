use futures::executor;
use openssl::{ssl::{SslMethod, SslConnector, SslStream}};
use std::{net::{UdpSocket, SocketAddr, Ipv4Addr, IpAddr}, io::{Write, Read, self}, num::ParseIntError};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Bridge {
    client: reqwest::Client,
    ip: Ipv4Addr,
    stream: SslStream<UStream>
}

#[derive(Debug)]
pub enum ConnectionError {
    Mode(reqwest::Error),
    Handshake(openssl::ssl::HandshakeError<UStream>)
}

impl Bridge {
    pub fn init() -> Result<Bridge, ConnectionError>{
        let client = reqwest::Client::builder().danger_accept_invalid_certs(true).build().unwrap();
        
        let app_key = "r7VTnaBOBJd6sB2FzTd3IU1hPs3ZocltQYDXQxXq";
        let app_id = "29385356-e7a3-4244-a8f6-33be42e95659";
        let area_id = "5fb0617b-4883-4a1b-86c4-c67b63a9d784";
        let psk = "50A65F0A61249C6ACFE7E95F3444BE71";
        let bridge_ip = "192.168.2.20".parse().unwrap();
        let psk_bytes = decode_hex(psk).unwrap();

        println!("Start Entertainment mode");
        match executor::block_on(start_entertainment_mode(&client, &bridge_ip, area_id, app_key)) {
            Ok(r) => {println!("{}", r.status())},
            Err(e) => {
                println!("Failed to start hue sync");
                println!("Error: {}", e);
                return Err(ConnectionError::Mode(e));
            }
        }
        println!("Building DTLS Connection");
        let mut builder = SslConnector::builder(SslMethod::dtls()).unwrap();
        builder.set_psk_client_callback(move |_, _, id, key| {
            app_id
                .as_bytes()
                .iter()
                .enumerate()
                .for_each(|(i, b)| id[i] = *b);
            psk_bytes
                .iter()
                .enumerate()
                .for_each(|(i, b)| key[i] = *b);
            Ok(0)
        });
        builder.set_ciphersuites("TLS_PSK_WITH_AES_128_GCM_SHA256").unwrap();
        let connector = builder.build();
        println!("Binding Socket");
        let socket = UdpSocket::bind("0.0.0.0:9549").unwrap();
        let stream = UStream{s: socket, ra: SocketAddr::new(IpAddr::V4(bridge_ip), 2100)};
        println!("Performing handshake");
        let stream = match connector.connect(bridge_ip.to_string().as_str(), stream){
            Ok(stream) => stream,
            Err(e) => return Err(ConnectionError::Handshake(e))
        };
        println!("Done");
        let bridge = Bridge {client, ip: bridge_ip, stream};
        return Ok(bridge);
    }
}

async fn start_entertainment_mode(client: &reqwest::Client, bridge_ip: &Ipv4Addr, area_id: &str, app_key: &str) -> Result<reqwest::Response, reqwest::Error>{
    client.put("https://".to_owned() + bridge_ip.to_string().as_str() + "/clip/v2/entertainment_configuration/" + area_id)
        .header("hue-application-key", app_key)
        .body("{\"action\":\"start\"}")
        .send().await
}

#[derive(Debug)]
pub struct UStream {
    pub s: UdpSocket,
    pub ra: SocketAddr,
}

impl Read for UStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.s.recv(buf)
    }
}

impl Write for UStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.s.send_to(buf, self.ra)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}