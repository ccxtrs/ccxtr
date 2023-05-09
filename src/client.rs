use std::fmt::Debug;
use serde::{Serialize};
use serde::de::DeserializeOwned;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::{Error, Result};

pub const NONE: Option<&'static ()> = None;

pub struct WsClient {
    client: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WsClient {
    pub async fn new<H: Into<String>>(endpoint: H) -> Result<Self> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(endpoint.into()).await?;
        Ok(Self {
            client: ws_stream,
        })
    }
}

pub struct HttpClient {
    host: String,
    port: u16,
    client: reqwest::Client,
}

impl HttpClient {
    pub async fn get<Q: Serialize, T: DeserializeOwned + Debug>(&self, endpoint: &str, query: Option<&Q>) -> Result<T> {
        let mut builder = self.client.get(format!("{}:{}{}", self.host, self.port, endpoint));
        if let Some(query) = query {
            builder = builder.query(query);
        }
        let response = builder.send().await?;
        let code = response.status();
        if code.as_u16() < 200 || code.as_u16() >= 300 {
            return Err(Error::HttpError(response.text().await?));
        }
        Ok(response.json::<T>().await?)
    }
}

#[derive(Default)]
pub struct Builder {
    host: String,
    port: u16,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            host: "".to_string(),
            port: 0,
        }
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn build(self) -> Result<HttpClient> {
        let result = reqwest::Client::builder().build();
        Ok(HttpClient {
            client: result?,
            host: self.host,
            port: self.port,
        })
    }
}