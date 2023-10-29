use std::collections::HashMap;

use async_trait::async_trait;
use tokio_stream::StreamExt;

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
use crate::error::*;
use crate::model::*;
use crate::util::channel::{Receiver, Sender};

mod binance;
mod property;
mod params;

#[derive(Clone)]
pub struct Unifier {
    unified_market_to_symbol_id: HashMap<Market, String>,
    symbol_id_to_unified_market: HashMap<String, Market>,

}

impl Unifier {
    pub fn new() -> Self {
        Self {
            unified_market_to_symbol_id: HashMap::new(),
            symbol_id_to_unified_market: HashMap::new(),
        }
    }

    pub fn insert_market_symbol_id(&mut self, market: &Market, symbol_id: &String) {
        self.unified_market_to_symbol_id.insert(market.clone(), symbol_id.clone());
        self.symbol_id_to_unified_market.insert(symbol_id.clone(), market.clone());
    }

    pub fn get_symbol_id(&self, market: &Market) -> Option<String> {
        self.unified_market_to_symbol_id.get(market).cloned()
    }

    pub fn get_market(&self, symbol_id: &String) -> Option<Market> {
        self.symbol_id_to_unified_market.get(symbol_id).cloned()
    }

    pub fn reset(&mut self) {
        self.unified_market_to_symbol_id.clear();
        self.symbol_id_to_unified_market.clear();
    }
}


pub enum StreamItem {
    OrderBook(OrderBookResult<OrderBook>),
}

pub struct ExchangeBase {
    pub(super) http_client: HttpClient,
    pub(super) ws_client: WsClient,
    pub(super) is_connected: bool,

    stream_parser: fn(&[u8], &Unifier) -> Option<StreamItem>,

    order_book_stream_tx: Sender<OrderBookResult<OrderBook>>,
    order_book_stream_rx: Receiver<OrderBookResult<OrderBook>>,

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
        let ws_client = WsClient::new(properties.ws_endpoint.clone().unwrap().as_str());
        let (order_book_stream_sender, order_book_stream) = async_broadcast::broadcast::<OrderBookResult<OrderBook>>(properties.channel_capacity.unwrap_or(1000));
        let order_book_stream_tx = Sender::new(order_book_stream_sender);
        let order_book_stream_rx = Receiver::new(order_book_stream);
        Ok(Self {
            markets: vec![],
            unifier: Unifier::new(),
            ws_client,
            http_client,
            stream_parser: properties.stream_parser.unwrap_or(|_, _| None),
            order_book_stream_tx,
            order_book_stream_rx,
            is_connected: false,
        })
    }

    pub(crate) async fn connect(&mut self) -> Result<()> {
        if self.markets.is_empty() {
            return Err(Error::MissingMarkets);
        }
        self.ws_client.connect().await?;
        let mut ws_rx = self.ws_client.receiver().ok_or(Error::WebsocketError("receiver is None".into()))?;
        let stream_parser = self.stream_parser;
        let order_book_stream_tx = self.order_book_stream_tx.clone();

        let unifier = self.unifier.clone();
        tokio::spawn(async move {
            loop {
                let message = ws_rx.next().await;
                match message {
                    Some(message) => {
                        match stream_parser(message.unwrap().as_slice(), &unifier) {
                            None => {
                                continue;
                            }
                            Some(StreamItem::OrderBook(order_book)) => {
                                let _ = order_book_stream_tx.send(order_book).await;
                            }
                        }
                    }
                    None => {
                        break;
                    }
                };
            }
        });
        self.is_connected = true;
        Ok(())
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
    async fn fetch_tickers(&self, _: &FetchTickersParams) -> FetchTickersResult<Vec<Ticker>> {
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
    async fn watch_order_book(&self, _: &WatchOrderBookParams) -> WatchOrderBookResult<Receiver<OrderBookResult<OrderBook>>> {
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
