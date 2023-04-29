use crate::error::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub use crate::exchange::Exchange;