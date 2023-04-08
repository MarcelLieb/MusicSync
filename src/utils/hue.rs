use futures::executor;
use openssl::ssl::{SslMethod, SslConnector};
use std::net::UdpSocket;

pub struct Bridge {
    client: reqwest::Client
}

impl Bridge {
    pub fn init() -> Result<Bridge, reqwest::Error>{
        let client = reqwest::Client::new();
        let bridge = Bridge {client};
        
        let app_key = "r7VTnaBOBJd6sB2FzTd3IU1hPs3ZocltQYDXQxXq";
        let area_id = "5fb0617b-4883-4a1b-86c4-c67b63a9d784";
        match executor::block_on(bridge.start_entertainment_mode(area_id, app_key)) {
            Ok(_) => {},
            Err(e) => {
                println!("Failed to start hue sync");
                println!("Error: {}", e);
                return Err(e);
            }
        }
        let mut builder = SslConnector::builder(SslMethod::dtls()).unwrap();
        builder.set_psk_client_callback(|ssl, hint, id, key| {
            Ok(0) 
        });
        let connector = builder.build();
        let socket = UdpSocket::bind("127.0.0.1:3999").unwrap();

        socket.connect("192.168.2.20:2100").expect("failed");
        return Ok(bridge);
    }

    async fn start_entertainment_mode(&self, area_id: &str, app_key: &str) -> Result<reqwest::Response, reqwest::Error>{
        self.client.put("https://192.168.2.20/clip/v2/entertainment_configuration/".to_owned() + area_id)
            .header("hue-application-key", app_key)
            .body("{\"action\":\"start\"}")
            .send().await
    }
}