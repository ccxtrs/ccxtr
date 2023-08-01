use std::collections::HashMap;

use async_trait::async_trait;
use futures::channel::mpsc;
use futures::StreamExt;

pub use binance::BinanceUsdm;
pub(in self) use property::Properties;
pub use property::PropertiesBuilder;

use crate::client::{HttpClient, HttpClientBuilder, WsClient};
use crate::error::Error;
use crate::model::{Currency, Market, Order, OrderBook, Trade};
use crate::Result;

mod binance;
mod property;

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
    OrderBook(Result<OrderBook>),
}

pub struct ExchangeBase {
    pub(super) http_client: HttpClient,
    pub(super) ws_client: WsClient,

    stream_parser: fn(Vec<u8>, &Unifier) -> Option<StreamItem>,

    order_book_stream_sender: mpsc::Sender<Result<OrderBook>>,
    order_book_stream: Option<mpsc::Receiver<Result<OrderBook>>>,

    pub(super) markets: Vec<Market>,

    pub(super) unifier: Unifier,
}

const OPEN_MASK: usize = usize::MAX - (usize::MAX >> 1);
const MAX_CAPACITY: usize = !(OPEN_MASK);
const MAX_BUFFER: usize = (MAX_CAPACITY >> 1) - 1;

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
            .error_parser(properties.error_parser)
            .build().unwrap();
        let ws_client = WsClient::new(properties.ws_endpoint.unwrap().as_str());
        let (order_book_stream_sender, order_book_stream) = mpsc::channel::<Result<OrderBook>>(MAX_BUFFER);
        Ok(Self {
            markets: vec![],
            unifier: Unifier::new(),
            ws_client,
            http_client,
            stream_parser: properties.stream_parser.unwrap_or(|_, _| None),
            order_book_stream_sender,
            order_book_stream: Some(order_book_stream),
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        if self.markets.is_empty() {
            return Err(Error::MissingMarkets);
        }
        self.ws_client.connect().await?;
        let stream_parser = self.stream_parser;
        let mut order_book_stream_sender = self.order_book_stream_sender.clone();
        let mut receiver = self.ws_client.receiver();

        let unifier = self.unifier.clone();
        tokio::spawn(async move {
            loop {
                let message = receiver.as_mut().unwrap().next().await;
                match message {
                    Some(message) => {
                        match stream_parser(message.unwrap(), &unifier) {
                            None => {
                                continue;
                            }
                            Some(StreamItem::OrderBook(order_book)) => {
                                let _ = order_book_stream_sender.try_send(order_book);
                            },
                        }
                    }
                    None => {
                        break;
                    }
                };
            }
        });

        Ok(())
    }
}

impl From<mpsc::SendError> for Error {
    fn from(e: mpsc::SendError) -> Self {
        Error::WebsocketError(format!("{}", e))
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
    async fn fetch_currencies(&self) -> Result<Vec<Currency>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_ticker(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_tickers(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_order_book(&self) -> Result<Vec<OrderBook>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_ohlcv(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_status(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn fetch_trades(&self) -> Result<Vec<Trade>> {
        Err(Error::NotImplemented)
    }

    async fn watch_ticker(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_tickers(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_order_book(&mut self, _: &Vec<Market>) -> Result<mpsc::Receiver<Result<OrderBook>>> {
        Err(Error::NotImplemented)
    }
    async fn watch_ohlcv(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_status(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn watch_trades(&self) -> Result<Result<mpsc::Receiver<Result<Trade>>>> {
        Err(Error::NotImplemented)
    }

    // private
    async fn fetch_balance(&self) -> Result<()> {
        Err(Error::NotImplemented)
    }
    async fn create_order(&self, _: Order) -> Result<Order> {
        Err(Error::NotImplemented)
    }
    async fn cancel_order(&self, _: Order) -> Result<Order> {
        Err(Error::NotImplemented)
    }
    async fn fetch_order(&self) -> Result<Order> {
        Err(Error::NotImplemented)
    }
    async fn fetch_orders(&self) -> Result<Vec<Order>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_open_orders(&self) -> Result<Vec<Order>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_closed_orders(&self) -> Result<Vec<Order>> {
        Err(Error::NotImplemented)
    }
    async fn fetch_my_trades(&self) -> Result<Vec<Trade>> {
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