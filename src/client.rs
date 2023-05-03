use std::fmt::Debug;
use serde::{Serialize};
use serde::de::DeserializeOwned;

use crate::Result;

pub const NONE: Option<&'static ()> = None;

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
        Ok(builder.send().await?.json::<T>().await?)
    }
}

#[derive(Default)]
pub struct Builder {
    host: String,
    port: u16,
}

impl Builder {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
        }
    }

    pub fn build(self) -> HttpClient {
        HttpClient {
            client: reqwest::Client::new(),
            host: self.host,
            port: self.port,
        }
    }
}