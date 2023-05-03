
#[derive(Debug)]
pub struct Properties {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) api_key: Option<String>,
    pub(crate) secret_key: Option<String>,
}

#[derive(Default)]
pub struct PropertiesBuilder {
    host: Option<String>,
    port: Option<u16>,
    api_key: Option<String>,
    secret_key: Option<String>,
}

impl PropertiesBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn host<S: Into<String>>(mut self, host: S) -> Self {
        self.host = Some(host.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn api_key<S: Into<String>>(mut self, api_key: S) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn secret_key<S: Into<String>>(mut self, secret_key: S) -> Self {
        self.secret_key = Some(secret_key.into());
        self
    }

    pub fn build(self) -> Properties {
        Properties {
            host: self.host,
            port: self.port,
            api_key: self.api_key,
            secret_key: self.secret_key,
        }
    }
}