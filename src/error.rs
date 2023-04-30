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
}