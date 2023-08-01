use std::fmt::Debug;
use std::io;

use futures::{SinkExt, StreamExt};
use futures::channel::mpsc::Sender;
use futures::stream::{Map, SplitStream};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, tungstenite, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;

use crate::{Error, Result};

pub const EMPTY_QUERY: Option<&'static ()> = None;
pub const EMPTY_BODY: Option<&String> = None;


type ReceiveStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
type MapFunc = fn(core::result::Result<Message, tungstenite::error::Error>) -> core::result::Result<Vec<u8>, Error>;
type MappedReceiveStream = Map<ReceiveStream, MapFunc>;

pub struct WsClient {
    endpoint: String,

    sender: Option<Sender<String>>,
    receiver: Option<MappedReceiveStream>,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::WebsocketError(format!("{}", e))
    }
}


const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);
const MAX_CAPACITY: usize = !(OPEN_MASK);
const MAX_BUFFER: usize = (MAX_CAPACITY >> 1) - 1;

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
        let (send_ch, mut recv_ch) = futures::channel::mpsc::channel::<String>(MAX_BUFFER);
        let sender = send_ch.clone();
        tokio::spawn(async move {
            while let Some(x) = recv_ch.next().await {
                let _ = tx.send(Message::from(x)).await;
            }
        });

        let rx: MappedReceiveStream = rx.map(|x| {
            match x {
                Ok(x) => Ok(x.into_data()),
                Err(e) => Err(Error::WebsocketError(format!("{}", e)))
            }
        });

        self.sender = Some(sender);
        self.receiver = Some(rx);
        Ok(self)
    }

    pub fn sender(&self) -> Option<Sender<String>> {
        self.sender.clone()
    }

    pub fn receiver(&mut self) -> Option<MappedReceiveStream> {
        self.receiver.take()
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
    error_parser: fn(message: String) -> Error,
}

impl HttpClient {
    /// query: `&[("foo", "a"), ("foo", "b")])` makes `"foo=a&foo=b"`
    pub async fn get<Q: Serialize + ?Sized, T: DeserializeOwned + Debug>(&self, endpoint: &str, query: Option<&Q>) -> Result<T> {
        let mut builder = self.client.get(format!("{}:{}{}", self.host, self.port, endpoint));
        if let Some(query) = query {
            builder = builder.query(query);
        }
        let response = builder.send().await?;
        let code = response.status();
        if !code.is_success() {
            return Err((self.error_parser)(response.text().await?));
        }
        Ok(response.json::<T>().await?)
    }

    pub async fn post<Q: Serialize + ?Sized, B: AsRef<str>, T: DeserializeOwned + Debug>(&self, endpoint: &str, headers: Option<Vec<(&str, &str)>>, query: Option<&Q>, body: Option<&B>) -> Result<T> {
        let mut builder = self.client.post(format!("{}:{}{}", self.host, self.port, endpoint));
        if let Some(query) = query {
            builder = builder.query(query);
        }

        if headers.is_some() {
            for (k, v) in headers.unwrap() {
                builder = builder.header(k, v);
            }
        }

        if let Some(body) = body {
            builder = builder.body(body.as_ref().to_owned());
        }
        let response = builder.send().await?;
        let code = response.status();
        if !code.is_success() {
            return Err((self.error_parser)(response.text().await?));
        }
        Ok(response.json::<T>().await?)
    }
}

#[derive(Default)]
pub struct HttpClientBuilder {
    host: String,
    port: u16,
    error_parser: Option<fn(message: String) -> Error>,
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        Self {
            host: "".to_string(),
            port: 0,
            error_parser: None,
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

    pub fn error_parser(mut self, handler: Option<fn(message: String) -> Error>) -> Self {
        self.error_parser = handler;
        self
    }

    pub fn build(self) -> Result<HttpClient> {
        let result = reqwest::Client::builder().build();
        Ok(HttpClient {
            client: result?,
            host: self.host,
            port: self.port,
            error_parser: self.error_parser.unwrap_or(|x| Error::HttpError(x))
        })
    }
}


#[cfg(test)]
mod test {
    use futures::prelude::*;

    use crate::client::WsClient;
    use crate::Error;

    #[tokio::test]
    async fn test_ws_client() {
        let mut client = WsClient::new("wss://stream.binance.com:9443/ws");
        client.connect().await.unwrap();
        let sender = client.sender();
        let mut sender = sender.unwrap();
        sender.send("test".to_string()).await.unwrap();
        sender.send("test2".to_string()).await.unwrap();

        let mut receiver = client.receiver();
        let msg = receiver.as_mut().unwrap().next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
        let msg = receiver.as_mut().unwrap().next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
    }



    #[tokio::test]
    async fn test_post() {
        let client = crate::client::HttpClientBuilder::new()
            .host("http://localhost".to_string())
            .port(3246)
            .build().unwrap();
        let result: Result<String, Error> = client.post("/test", crate::client::EMPTY_QUERY, crate::client::EMPTY_BODY).await;
        println!("{:?}", result);
    }
}
