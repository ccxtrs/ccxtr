use std::fmt::Debug;
use std::io;

use futures_util::{SinkExt, StreamExt};
use futures_util::stream::{Map, SplitStream};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, tungstenite, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{Error, Result};

pub(crate) const EMPTY_QUERY: Option<&'static ()> = None;
pub(crate) const EMPTY_BODY: Option<&String> = None;


type ReceiveStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;
type MapFunc = fn(core::result::Result<Message, tungstenite::error::Error>) -> core::result::Result<Vec<u8>, Error>;
type MappedReceiveStream = Map<ReceiveStream, MapFunc>;

pub(crate) struct WsClient {
    endpoint: String,

    sender: Option<flume::Sender<String>>,
    receiver: Option<MappedReceiveStream>,
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

    pub(crate) async fn connect(&mut self) -> Result<&Self> {
        let (stream, _) = connect_async(self.endpoint.as_str()).await.expect("Failed to connect");
        let (mut ws_tx, ws_rx) = stream.split();
        let (req_tx, req_rx) = flume::unbounded::<String>();
        tokio::spawn({
            async move {
                loop {
                    let req = req_rx.recv_async().await;
                    match req {
                        Ok(x) => {
                            let _ = ws_tx.send(Message::Text(x)).await;
                        }
                        _ => {
                            break;
                        }
                    }
                }
            }
        });

        let rx: MappedReceiveStream = ws_rx.map(|x| {
            match x {
                Ok(x) => Ok(x.into_data()),
                Err(e) => Err(Error::WebsocketError(format!("{}", e)))
            }
        });

        self.sender = Some(req_tx);
        self.receiver = Some(rx);
        Ok(self)
    }

    pub(crate) fn sender(&self) -> Option<flume::Sender<String>> {
        self.sender.clone()
    }

    pub(crate) fn receiver(&mut self) -> Option<MappedReceiveStream> {
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
    pub(crate) async fn get<Q: Serialize + ?Sized, T: DeserializeOwned + Debug>(&self, endpoint: &str, headers: Option<Vec<(&str, &str)>>, query: Option<&Q>) -> Result<T> {
        let mut builder = self.client.get(format!("{}:{}{}", self.host, self.port, endpoint));
        if let Some(query) = query {
            builder = builder.query(query);
        }

        if headers.is_some() {
            for (k, v) in headers.unwrap() {
                builder = builder.header(k, v);
            }
        }

        let response = builder.send().await?;
        let code = response.status();
        if !code.is_success() {
            return Err((self.error_parser)(response.text().await?));
        }
        Ok(response.json::<T>().await?)
    }

    pub(crate) async fn post<Q: Serialize + ?Sized, B: AsRef<str>, T: DeserializeOwned + Debug>(&self, endpoint: &str, headers: Option<Vec<(&str, &str)>>, query: Option<&Q>, body: Option<&B>) -> Result<T> {
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
    pub(crate) fn new() -> Self {
        Self {
            host: "".to_string(),
            port: 0,
            error_parser: None,
        }
    }

    pub(crate) fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub(crate) fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub(crate) fn error_parser(mut self, handler: Option<fn(message: String) -> Error>) -> Self {
        self.error_parser = handler;
        self
    }

    pub(crate) fn build(self) -> Result<HttpClient> {
        let result = reqwest::Client::builder().build();
        Ok(HttpClient {
            client: result?,
            host: self.host,
            port: self.port,
            error_parser: self.error_parser.unwrap_or(|x| Error::HttpError(x)),
        })
    }
}


#[cfg(test)]
mod test {
    use tokio_stream::StreamExt;

    use crate::client::WsClient;

    #[tokio::test]
    async fn test_ws_client() {
        let mut client = WsClient::new("wss://stream.binance.com:9443/ws");
        client.connect().await.unwrap();
        let sender = client.sender();
        let sender = sender.unwrap();
        sender.send_async("test".to_string()).await.unwrap();
        sender.send_async("test2".to_string()).await.unwrap();

        let mut receiver = client.receiver();
        let receiver = receiver.as_mut().unwrap();
        let msg = receiver.next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
        let msg = receiver.next().await.unwrap();
        println!("{:?}", String::from_utf8(msg.unwrap()).unwrap());
    }
}
