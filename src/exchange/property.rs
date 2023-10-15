use derive_builder::Builder;

use crate::error::Error;
use crate::exchange::{StreamItem, Unifier};

#[derive(Default, Builder, Debug)]
#[builder(default)]
pub struct Properties {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub api_key: Option<String>,
    pub secret: Option<String>,
    pub ws_endpoint: Option<String>,
    pub channel_capacity: Option<usize>,
}


#[derive(Default, Builder, Debug)]
#[builder(default)]
pub(crate) struct BaseProperties {
    pub(crate) host: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) ws_endpoint: Option<String>,
    pub(crate) stream_parser: Option<fn(Vec<u8>, &Unifier) -> Option<StreamItem>>,
    pub(crate) error_parser: Option<fn(String) -> Error>,
    pub(crate) channel_capacity: Option<usize>,
}
