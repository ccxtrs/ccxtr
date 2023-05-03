use std::num::ParseFloatError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("not implemented")]
    NotImplemented,
    #[error("deserialization error for json body {0}")]
    DeserializeJsonBody(String),
    #[error("http error {source}")]
    HttpError {
        #[from]
        source: reqwest::Error
    },
    #[error("missing field! {0}")]
    MissingField(String),
    #[error("parse error {0}")]
    ParseError(String),
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