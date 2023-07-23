mod binance;
mod property;

use std::collections::HashMap;
use crate::error::Error;
use crate::Result;
use crate::model::{Market, OrderBook};

pub use binance::BinanceUsdm;
pub(in self) use property::Properties;
pub use property::PropertiesBuilder;


use async_trait::async_trait;
use futures::Stream;
use tokio::sync::RwLock;
use crate::client::{HttpClient, HttpClientBuilder, WsClient};

pub struct Unifier {
    unified_market_to_symbol_id: RwLock<HashMap<Market, String>>,
    symbol_id_to_unified_market: RwLock<HashMap<String, Market>>,

}

impl Unifier {
    pub fn new() -> Self {
        Self {
            unified_market_to_symbol_id: RwLock::new(HashMap::new()),
            symbol_id_to_unified_market: RwLock::new(HashMap::new()),
        }
    }

    pub async fn insert_market_symbol_id(&self, market: &Market, symbol_id: &String) {
        self.unified_market_to_symbol_id.write().await.insert(market.clone(), symbol_id.clone());
        self.symbol_id_to_unified_market.write().await.insert(symbol_id.clone(), market.clone());
    }

    pub async fn get_symbol_id(&self, market: &Market) -> Option<String> {
        self.unified_market_to_symbol_id.read().await.get(market).cloned()
    }
}



pub struct ExchangeBase {
    pub http_client: HttpClient,
    pub ws_client: WsClient,

    pub markets: Vec<Market>,

    pub unifier: Unifier,
}

impl ExchangeBase {
    pub fn new(properties: Properties) -> Result<Self> {
        if properties.host.is_none() {
            return Err(Error::MissingProperties("host".into()));
        }
        if properties.port.is_none() {
            return Err(Error::MissingProperties("port".into()));
        }
        if properties.ws_endpoint.is_none() {
            return Err(Error::MissingProperties("ws_endpoint".into()));
        }
        let http_client = HttpClientBuilder::new()
            .host(properties.host.unwrap())
            .port(properties.port.unwrap())
            .build().unwrap();
        let mut ws_client = WsClient::new(properties.ws_endpoint.unwrap().as_str());
        Ok(Self {
            markets: vec![],
            unifier: Unifier::new(),
            ws_client,
            http_client,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.ws_client.connect().await?;
        Ok(())
    }
}

#[async_trait]
pub trait Exchange {
    // public
    async fn load_markets(&mut self) -> Result<&Vec<Market>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_markets(&mut self) -> Result<&Vec<Market>> {
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
    async fn watch_order_book(&mut self, markets: Vec<Market>) -> Result<Box<dyn Stream<Item=OrderBook> + Unpin + '_>> {
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