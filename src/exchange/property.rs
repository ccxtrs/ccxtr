use std::sync::{Arc, RwLock};

use crate::error::Error;
use crate::exchange::{StreamItem, Unifier};
use crate::util::OrderBookSynchronizer;

#[derive(Debug)]
pub struct Properties {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) api_key: Option<String>,
    pub(crate) secret: Option<String>,
    pub(crate) ws_endpoint: Option<String>,
    pub(crate) stream_parser: Option<fn(Vec<u8>, &Unifier, &Arc<RwLock<OrderBookSynchronizer>>) -> Option<StreamItem>>,
    pub(crate) error_parser: Option<fn(String) -> Error>,
}

#[derive(Default)]
pub struct PropertiesBuilder {
    host: Option<String>,
    port: Option<u16>,
    api_key: Option<String>,
    secret: Option<String>,
    ws_endpoint: Option<String>,
    stream_parser: Option<fn(Vec<u8>, &Unifier, &Arc<RwLock<OrderBookSynchronizer>>) -> Option<StreamItem>>,
    error_parser: Option<fn(String) -> Error>,
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

    pub fn secret<S: Into<String>>(mut self, secret_key: S) -> Self {
        self.secret = Some(secret_key.into());
        self
    }

    pub fn ws_endpoint<S: Into<String>>(mut self, ws_endpoint: S) -> Self {
        self.ws_endpoint = Some(ws_endpoint.into());
        self
    }

    pub(crate) fn stream_parser(mut self, stream_parser: fn(Vec<u8>, &Unifier, &Arc<RwLock<OrderBookSynchronizer>>) -> Option<StreamItem>) -> Self {
        self.stream_parser = Some(stream_parser);
        self
    }

    pub(crate) fn error_parser(mut self, error_parser: fn(String) -> Error) -> Self {
        self.error_parser = Some(error_parser);
        self
    }

    pub fn build(self) -> Properties {
        Properties {
            host: self.host,
            port: self.port,
            api_key: self.api_key,
            secret: self.secret,
            ws_endpoint: self.ws_endpoint,
            stream_parser: self.stream_parser,
            error_parser: self.error_parser,
        }
    }
}