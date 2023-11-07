use async_trait::async_trait;

pub use binance::Binance;
pub use binance::BinanceUsdm;
pub use params::{WatchOrderBookParams, WatchOrderBookParamsBuilder, WatchOrderBookParamsBuilderError};
pub use params::{FetchBalanceParams, FetchBalanceParamsBuilder, FetchBalanceParamsBuilderError};
pub use params::{FetchTickersParams, FetchTickersParamsBuilder, FetchTickersParamsBuilderError};
pub use params::{CreateOrderParams, CreateOrderParamsBuilder, CreateOrderParamsBuilderError};
pub use params::{FetchPositionsParams, FetchPositionsParamsBuilder, FetchPositionsParamsBuilderError};
pub use property::{Properties, PropertiesBuilder, PropertiesBuilderError};
pub(crate) use property::{BaseProperties, BasePropertiesBuilder, BasePropertiesBuilderError};

use crate::client::{HttpClient, HttpClientBuilder, WsClient};
pub(crate) use crate::exchange::unifier::Unifier;
use crate::error::*;
use crate::model::*;
use crate::util::channel::Receiver;

mod binance;
mod property;
mod params;

mod unifier;


#[derive(Debug, Clone)]
pub enum StreamItem {
    OrderBook(OrderBookResult<OrderBook>),
    Subscribed(i64),
    Unknown(String),
}

pub struct ExchangeBase {
    pub(super) http_client: HttpClient,
    pub(super) ws_endpoint: Option<String>,

    stream_parser: fn(&[u8], &Unifier) -> WatchResult<Option<StreamItem>>,

    pub(super) markets: Vec<Market>,

    pub(super) unifier: Unifier,
}


impl ExchangeBase {
    pub(crate) fn new(properties: &BaseProperties) -> Result<Self> {
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
            .host(properties.host.clone().unwrap())
            .port(properties.port.clone().unwrap())
            .error_parser(properties.error_parser)
            .build().unwrap();
        Ok(Self {
            markets: vec![],
            unifier: Unifier::new(),
            ws_endpoint: properties.ws_endpoint.clone(),
            http_client,
            stream_parser: properties.stream_parser.unwrap_or(|_, _| Ok(None)),
        })
    }
}


#[async_trait]
pub trait Exchange {
    /// Load all markets from the exchange and store them in the internal cache.
    ///
    /// It also updates the internal unifier which is used to convert market to symbol id and vice
    /// versa.
    async fn load_markets(&mut self) -> LoadMarketResult<Vec<Market>> {
        Err(LoadMarketError::NotImplemented)
    }

    async fn fetch_markets(&self) -> FetchMarketResult<Vec<Market>> {
        Err(FetchMarketError::NotImplemented)
    }
    async fn fetch_currencies(&self) -> CommonResult<Vec<Currency>> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_ticker(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_tickers(&self, _: FetchTickersParams) -> FetchTickersResult<Vec<Ticker>> {
        Err(FetchTickersError::NotImplemented)
    }
    async fn fetch_order_book(&self) -> CommonResult<Vec<OrderBook>> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_ohlcv(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_status(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_trades(&self) -> CommonResult<Vec<Trade>> {
        Err(CommonError::NotImplemented)
    }

    async fn watch_ticker(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn watch_tickers(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn watch_order_book(&self, _: &WatchOrderBookParams) -> WatchOrderBookResult<Receiver> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_ohlcv(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_status(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_trades(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }

    // private
    async fn fetch_balance(&self, _: &FetchBalanceParams) -> FetchBalanceResult<Balance> {
        Err(FetchBalanceError::NotImplemented)
    }

    async fn fetch_positions(&self, _: &FetchPositionsParams) -> FetchPositionsResult<Vec<Position>> {
        Err(FetchPositionsError::NotImplemented)
    }

    async fn create_order(&self, _: &CreateOrderParams) -> CreateOrderResult<Order> {
        Err(CreateOrderError::NotImplemented)
    }
    async fn cancel_order(&self, _: Order) -> CommonResult<Order> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_order(&self) -> CommonResult<Order> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_orders(&self) -> CommonResult<Vec<Order>> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_open_orders(&self) -> CommonResult<Vec<Order>> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_closed_orders(&self) -> CommonResult<Vec<Order>> {
        Err(CommonError::NotImplemented)
    }
    async fn fetch_my_trades(&self) -> CommonResult<Vec<Trade>> {
        Err(CommonError::NotImplemented)
    }
    async fn deposit(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }
    async fn withdraw(&self) -> CommonResult<()> {
        Err(CommonError::NotImplemented)
    }

    async fn watch_balance(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_my_trades(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_orders(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
    async fn watch_positions(&self) -> WatchResult<()> {
        Err(WatchError::NotImplemented)
    }
}
