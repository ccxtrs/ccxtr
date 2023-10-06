pub use exchange::BinanceMargin;
pub use exchange::BinanceUsdm;
pub use exchange::Exchange;
pub use exchange::FetchBalanceParams;
pub use exchange::Properties;
pub use exchange::PropertiesBuilder;

pub use crate::error::{ConnectError, ConnectResult};
pub use crate::error::{FetchMarketError, FetchMarketResult};
pub use crate::error::{LoadMarketError, LoadMarketResult};
pub use crate::error::{CommonError, CommonResult};
pub use crate::error::{WatchError, WatchResult};
pub use crate::error::{OrderBookError, OrderBookResult};
pub use crate::error::{CreateOrderError, CreateOrderResult};
pub use crate::util::channel::Receiver;

mod exchange;
mod client;
mod error;
mod util;

pub mod model;


