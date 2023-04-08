use futures::executor;

pub struct Bridge {

}

impl Bridge {
    pub async fn init() {
        let client = reqwest::Client::new();
        let response = executor::block_on(send_put(client));
        match response {
            Ok(response) => {
                println!("Status: {}", response.status());
                println!("Headers:\n{:#?}", response.headers());
                let body = response.text().await;
                match body {
                    Ok(body) => {
                        println!("Body:\n{}", body);
                    },
                    Err(err) => {
                        println!("Error: {}", err);
                    }
                };
            },
            Err(err) => {
                println!("Error: {}", err);
            }
        }
    }
}

async fn send_put(client: reqwest::Client) -> Result<reqwest::Response, reqwest::Error>{
    client.put("https://192.168.2.20/clip/v2/entertainment_configuration/5fb0617b-4883-4a1b-86c4-c67b63a9d784")
        .header("hue-application-key", "r7VTnaBOBJd6sB2FzTd3IU1hPs3ZocltQYDXQxXq")
        .body("{\"action\":\"start\"}")
        .send().await
}