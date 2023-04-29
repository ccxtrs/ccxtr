mod usdm;
pub use usdm::BinanceUsdm;


#[derive(Debug)]
pub struct Properties {
    host: String,
    port: u16,
    api_key: String,
    secret_key: String,
}

impl Properties {
    pub fn builder() -> PropertiesBuilder {
        PropertiesBuilder::default()
    }
}

#[derive(Default)]
pub struct PropertiesBuilder {
    host: Option<String>,
    port: Option<u16>,
    api_key: Option<String>,
    secret_key: Option<String>,
}

impl PropertiesBuilder {
    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = Some(host);
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn secret_key<S: Into<String>>(mut self, secret_key: S) -> Self {
        self.secret_key = Some(secret_key);
        self
    }

    pub fn build(self) -> Properties {
        Properties {
            host: self.host.unwrap_or_else(|| "api.binance.com".to_string()),
            port: self.port.unwrap_or(443),
            api_key: self.api_key.unwrap_or_else(|| "".to_string()),
            secret_key: self.secret_key.unwrap_or_else(|| "".to_string()),
        }
    }
}