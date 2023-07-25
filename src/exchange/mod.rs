use std::collections::HashMap;

use async_trait::async_trait;
use futures::channel::mpsc;
use futures::StreamExt;
use tokio::sync::RwLock;

pub use binance::BinanceUsdm;
pub(in self) use property::Properties;
pub use property::PropertiesBuilder;

use crate::client::{HttpClient, HttpClientBuilder, WsClient};
use crate::error::Error;
use crate::model::{Market, OrderBook};
use crate::Result;

mod binance;
mod property;

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

pub enum StreamItem {
    None,
    OrderBook(OrderBook),
}

pub struct ExchangeBase {
    pub(super) http_client: HttpClient,
    pub(super) ws_client: WsClient,

    stream_parser: fn(Vec<u8>) -> StreamItem,

    order_book_stream_sender: mpsc::Sender<OrderBook>,
    order_book_stream: Option<mpsc::Receiver<OrderBook>>,

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
            .build().unwrap();
        let ws_client = WsClient::new(properties.ws_endpoint.unwrap().as_str());
        let (order_book_stream_sender, order_book_stream) = mpsc::channel::<OrderBook>(MAX_BUFFER);
        let order_book_stream = Some(order_book_stream);
        Ok(Self {
            markets: vec![],
            unifier: Unifier::new(),
            ws_client,
            http_client,
            stream_parser: properties.stream_parser.unwrap_or(|_| StreamItem::None),
            order_book_stream_sender,
            order_book_stream,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.ws_client.connect().await?;
        let stream_parser = self.stream_parser;
        let mut order_book_stream_sender = self.order_book_stream_sender.clone();
        let mut receiver = self.ws_client.receiver();

        tokio::spawn(async move {
            loop {
                let message = receiver.as_mut().unwrap().next().await;
                match message {
                    Some(message) => {
                        match stream_parser(message.unwrap()) {
                            StreamItem::None => {
                                continue;
                            }
                            StreamItem::OrderBook(order_book) => {
                                order_book_stream_sender.try_send(order_book).unwrap();
                            }
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
    async fn watch_order_book(&mut self, _: Vec<Market>) -> Result<mpsc::Receiver<OrderBook>> {
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