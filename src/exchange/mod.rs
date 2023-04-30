mod binance;
mod property;

use crate::error::Error;
use crate::Result;
use crate::model::Market;

pub use binance::BinanceUsdm;
pub use property::Properties;


use async_trait::async_trait;

#[async_trait]
pub trait Exchange {
    // public
    async fn load_markets(&self) -> Result<Vec<Market>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_markets(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_currencies(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_ticker(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_tickers(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_order_book(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_ohlcv(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_status(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_trades(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }

    async fn watch_ticker(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_tickers(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_order_book(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_ohlcv(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_status(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_trades(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }

    // private
    async fn fetch_balance(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn create_order(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn cancel_order(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_order(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_orders(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_open_orders(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_closed_orders(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_my_trades(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn deposit(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn withdraw(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }

    async fn watch_balance(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_my_trades(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_orders(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_positions(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
}