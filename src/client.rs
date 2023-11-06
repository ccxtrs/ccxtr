
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{SinkExt, Stream, StreamExt};
use futures_util::stream::{Map, SplitStream};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, MaybeTlsStream, tungstenite, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;

use crate::error::{Error, Result};
use crate::exchange::{StreamItem, Unifier};
use crate::{WatchError, WatchResult};

pub(crate) const EMPTY_QUERY: Option<&'static ()> = None;
pub(crate) const EMPTY_BODY: Option<&String> = None;


pub(crate) struct WsClient {
    endpoint: String,
    parser: fn(&[u8], &Unifier) -> WatchResult<Option<StreamItem>>,
    unifier: Unifier,

    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::WebsocketError(format!("{}", e))
    }
}


impl Stream for WsClient {
    type Item = WatchResult<Option<StreamItem>>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.stream.as_mut().unwrap().poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(x))) => {
                let resp = (self.parser)(x.into_data().as_slice(), &self.unifier);
                Poll::Ready(Some(resp))
            },
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(WatchError::WebsocketError(format!("{}", e))))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl WsClient {
    pub fn new(endpoint: &str, parser: fn(&[u8], &Unifier) -> WatchResult<Option<StreamItem>>, unifier: Unifier) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            parser,
            unifier,
            stream: None,
        }
    }

    pub(crate) async fn send(&mut self, msg: String) -> Result<impl Stream + '_> {
        if self.stream.is_none() {
            let (stream, response) = connect_async(self.endpoint.as_str()).await.expect("Failed to connect");
            if response.status() != tungstenite::http::StatusCode::SWITCHING_PROTOCOLS {
                return Err(Error::WebsocketError(format!("Invalid status code: {}", response.status())));
            }
            self.stream = Some(stream);
        }
        self.stream.as_mut().unwrap().send(Message::Text(msg)).await?;
        Ok(self)
    }

    // pub(crate) async fn connect(&mut self) -> Result<&Self> {
    //     let (stream, _) = connect_async(self.endpoint.as_str()).await.expect("Failed to connect");
    //     let (mut ws_tx, mut ws_rx) = stream.split();
    //     let (tx, rx) = flume::unbounded::<String>();
    //     tokio::spawn({
    //         let rx = rx.clone();
    //         async move {
    //             loop {
    //                 let req = rx.recv_async().await;
    //                 match req {
    //                     Ok(x) => {
    //                         let _ = ws_tx.send(Message::Text(x)).await;
    //                     }
    //                     _ => {
    //                         break;
    //                     }
    //                 }
    //             }
    //         }
    //     });
    //
    //     tokio::spawn({
    //         let tx = tx.clone();
    //         async move {
    //             loop {
    //                 let resp = ws_rx.next().await;
    //                 match resp {
    //                     Some(Ok(x)) => {
    //                         let _ = tx.send_async(x.to_string()).await;
    //                     }
    //                     _ => {
    //                         break;
    //                     }
    //                 }
    //             }
    //         }
    //     });
    //
    //     self.sender = Some(tx);
    //     self.receiver = Some(rx);
    //     Ok(self)
    // }

    // pub(crate) fn sender(&self) -> Option<flume::Sender<String>> {
    //     self.sender.clone()
    // }
    //
    // pub(crate) fn receiver(&mut self) -> Option<flume::Receiver<String>> {
    //     self.receiver.clone()
    // }
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
    pub(crate) async fn get<Q: Serialize + ?Sized, T: DeserializeOwned>(&self, endpoint: &str, headers: Option<Vec<(&str, &str)>>, query: Option<&Q>) -> Result<T> {
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

    pub(crate) async fn post<Q: Serialize + ?Sized, B: AsRef<str>, T: DeserializeOwned>(&self, endpoint: &str, headers: Option<Vec<(&str, &str)>>, query: Option<&Q>, body: Option<&B>) -> Result<T> {
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
    use futures_util::StreamExt;
    use crate::client::WsClient;
    use crate::exchange::{StreamItem, Unifier};

    #[tokio::test]
    async fn test_ws_client() {
        let parser = |x: &[u8], _: &Unifier| {
            Ok(Some(StreamItem::Unknown(x.to_vec())))
        };

        let unifier = crate::exchange::Unifier::new();
        let mut client = WsClient::new("wss://stream.binance.com:9443/ws", parser, unifier);
        client.send("test".to_string()).await.unwrap();

        let resp = client.next().await.unwrap().unwrap().unwrap();
        match resp {
            StreamItem::Unknown(x) => {
                println!("{:?}", x);
            }
            _ => {}
        }
    }
}
