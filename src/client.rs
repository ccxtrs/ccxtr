use std::fmt::Debug;
use std::io;

use futures::{SinkExt, Stream, StreamExt};
use futures::channel::mpsc::Sender;
use futures::stream::SplitStream;
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;

use crate::{Error, Result};

pub const NONE: Option<&'static ()> = None;


pub struct WsClient {
    endpoint: String,

    sender: Option<Sender<String>>,
    receiver: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::WebsocketError(format!("{}", e))
    }
}

impl WsClient {
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            sender: None,
            receiver: None,
        }
    }

    pub async fn connect(&mut self) -> Result<&Self> {
        let (stream, _) = connect_async(self.endpoint.as_str()).await.expect("Failed to connect");
        let (mut tx, rx) = stream.split();
        let (send_ch, mut recv_ch) = futures::channel::mpsc::channel::<String>(1000);
        let sender = send_ch.clone();
        tokio::spawn(async move {
            while let Some(x) = recv_ch.next().await {
                println!("Sending: {}", x);
                let _ = tx.send(Message::from(x)).await;
            }
        });

        self.sender = Some(sender);
        self.receiver = Some(rx);
        Ok(self)
    }

    pub fn sender(&self) -> Result<Sender<String>> {
        match &self.sender {
            Some(sender) => Ok(sender.clone()),
            None => Err(Error::WebsocketError("No sender".to_string()))
        }
    }

    pub fn receiver(&mut self) -> impl Stream<Item=Result<Vec<u8>>> + '_ {
        self.receiver.as_mut().unwrap()
            .map(|x| {
                match x {
                    Ok(x) => Ok(x.into_data()),
                    Err(e) => Err(Error::WebsocketError(format!("{}", e)))
                }
            })
    }
}


impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebsocketError(format!("{}", e))
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
pub struct HttpClientBuilder {
    host: String,
    port: u16,
}

impl HttpClientBuilder {
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


#[cfg(test)]
mod test {
    use futures::prelude::*;

    use crate::client::WsClient;

    #[tokio::test]
    async fn test_ws_client() {
        let mut client = WsClient::new("wss://stream.binance.com:9443/ws");
        client.connect().await.unwrap();
        let sender = client.sender();
        let mut sender = sender.unwrap();
        sender.send("test".to_string()).await.unwrap();
        sender.send("test2".to_string()).await.unwrap();

        let msg = client.receiver().next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());

        let msg = client.receiver().next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
        let msg = client.receiver().next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
    }
}
