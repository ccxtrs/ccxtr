mod exchange;
mod client;
mod error;
mod util;

pub mod model;


pub use crate::error::{FetchMarketResult, FetchMarketError};
pub use crate::error::{LoadMarketResult, LoadMarketError};
pub use crate::error::{CommonResult, CommonError};
pub use crate::error::{WatchResult, WatchError};
pub use crate::error::{OrderBookResult, OrderBookError};
pub use crate::error::{CreateOrderResult, CreateOrderError};


pub use exchange::Exchange;
pub use exchange::PropertiesBuilder;
pub use exchange::Properties;

pub use exchange::BinanceUsdm;
pub use exchange::BinanceMargin;

pub use futures::StreamExt;
pub use futures::channel::mpsc::Receiver;

