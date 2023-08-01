use std::num::ParseFloatError;
use hmac::digest::InvalidLength;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not implemented")]
    NotImplemented,
    #[error("deserialization error for json body {0}")]
    DeserializeJsonBody(String),
    #[error("http error {0}")]
    HttpError(String),
    #[error("websocket error {0}")]
    WebsocketError(String),
    #[error("missing field {0}")]
    MissingField(String),
    #[error("parse error {0}")]
    ParseError(String),
    #[error("missing properties {0}")]
    MissingProperties(String),
    #[error("symbol not found {0}")]
    SymbolNotFound(String),
    #[error("missing credentials")]
    MissingCredentials,
    #[error("fetch markets first")]
    MissingMarkets,
    #[error("missing price")]
    MissingPrice,

    #[error("unsupported order type {0}")]
    UnsupportedOrderType(String),
    #[error("unsupported order side {0}")]
    UnsupportedOrderSide(String),
    #[error("unsupported order status {0}")]
    UnsupportedOrderStatus(String),
    #[error("unsupported time in force {0}")]
    UnsupportedTimeInForce(String),
    #[error("credentials error {0}")]
    CredentialsError(String),

    #[error("invalid order book {0}")]
    InvalidOrderBook(String),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::HttpError(format!("{}", e))
    }
}

impl From<ParseFloatError> for Error {
    fn from(e: ParseFloatError) -> Self {
        Error::ParseError(format!("{}", e))
    }
}

impl From<InvalidLength> for Error {
    fn from(value: InvalidLength) -> Self {
        Error::CredentialsError(format!("{}", value))
    }
}