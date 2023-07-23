use std::num::ParseFloatError;
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
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::HttpError(format!("{}", e))
    }
}

impl From<rust_decimal::Error> for Error {
    fn from(e: rust_decimal::Error) -> Self {
        Error::ParseError(format!("{}", e))
    }
}

impl From<ParseFloatError> for Error {
    fn from(e: ParseFloatError) -> Self {
        Error::ParseError(format!("{}", e))
    }
}
