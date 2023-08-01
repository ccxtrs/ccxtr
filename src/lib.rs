mod exchange;
pub mod client;
mod error;
mod util;

pub mod model;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

pub use exchange::Exchange;
pub use exchange::BinanceUsdm;
pub use exchange::PropertiesBuilder;

pub use futures::StreamExt;

