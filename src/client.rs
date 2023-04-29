struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn get<S: Into<String>>(&self, endpoint: S) -> reqwest::RequestBuilder {
        let result = self.client.get(endpoint.into()).send().await;
        result
    }
}

#[derive(Default)]
struct Builder {
    host: Option<String>,
    port: Option<u16>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            host: None,
            port: None,
        }
    }

    pub fn build(self) -> HttpClient {
        HttpClient {
            client: self.builder.build().unwrap(),
        }
    }
}