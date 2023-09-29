use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{CommonResult, CreateOrderResult, Exchange, FetchMarketResult, LoadMarketResult, OrderBookError, OrderBookResult, PropertiesBuilder, WatchError, WatchResult};
use crate::client::{EMPTY_BODY, EMPTY_QUERY};
use crate::error::{ConnectError, ConnectResult, Error, Result};
use crate::exchange::{ExchangeBase, StreamItem};
use crate::exchange::binance::util;
use crate::exchange::property::Properties;
use crate::model::{Market, MarketLimit, MarketType, Order, OrderBook, OrderBookUnit, OrderStatus, OrderType, Precision, Range, TimeInForce};
use crate::util::{into_precision, OrderBookDiff};
use crate::util::channel::Receiver;

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    code: i64,
    msg: String,
}

pub struct BinanceMargin {
    exchange_base: ExchangeBase,
    api_key: Option<String>,
    secret: Option<String>,
}

impl BinanceMargin {
    pub fn new(props: &Properties) -> CommonResult<Self> {
        let common_props = PropertiesBuilder::new()
            .host(props.host.as_ref().map_or("https://api.binance.com", |s| s.as_str()))
            .port(props.port.unwrap_or(443))
            .ws_endpoint("wss://stream.binance.com:9443/ws")
            .error_parser(|message| {
                match serde_json::from_str::<ErrorResponse>(&message) {
                    Ok(error) => {
                        match error.code {
                            -2019 => Error::InsufficientMargin(error.msg), // Margin is insufficient
                            -1013 => Error::InvalidQuantity(error.msg), // Invalid quantity
                            -1021 => Error::HttpError(error.msg), // Timestamp for this request is outside of the recvWindow
                            -1022 => Error::InvalidSignature(error.msg), // Signature for this request is not valid
                            -1100 => Error::InvalidParameters(error.msg), // Illegal characters found in a parameter
                            -1101 => Error::InvalidParameters(error.msg), // Too many parameters sent for this endpoint
                            _ => Error::HttpError(error.msg),
                        }
                    }
                    Err(_) => Error::DeserializeJsonBody(message),
                }
            })
            .stream_parser(|message, unifier, synchronizer| {
                let common_message = WatchCommonResponse::try_from(message.clone()).ok()?;
                if common_message.result.is_some() { // subscription result
                    return None;
                }
                match common_message.event_type {
                    Some(event_type) if event_type == "depthUpdate" => {
                        let resp = WatchDiffOrderBookResponse::from(message);
                        let market = unifier.get_market(&resp.symbol);
                        if market.is_none() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Unknown market. symbol={}", resp.symbol), None,
                            ))));
                        }
                        let market = market.unwrap();
                        let bids = resp.bids.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if bids.is_err() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid bid. bid={:?}", resp.bids), Some(market),
                            ))));
                        }
                        let asks = resp.asks.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if asks.is_err() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid ask. ask={:?}", resp.asks), Some(market),
                            ))));
                        }

                        let diff = OrderBookDiff::new(resp.first_update_id, resp.final_update_id, bids.unwrap(), asks.unwrap(), Some(resp.event_time));
                        let result = synchronizer.read().unwrap().append_and_get(market.clone(), diff);
                        if result.is_err() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid order book. error={:?}", result.expect_err("")), Some(market),
                            ))));
                        }
                        let book = result.unwrap();
                        if book.is_none() {
                            return None;
                        }
                        let book = book.unwrap();
                        Some(StreamItem::OrderBook(Ok(book)))
                    }
                    _ => return None,
                }
            });

        Ok(Self {
            exchange_base: ExchangeBase::new(&common_props.build())?,
            api_key: props.api_key.clone(),
            secret: props.secret.clone(),
        })
    }
    async fn connect(&mut self) -> CommonResult<()> {
        self.exchange_base.connect().await?;
        Ok(())
    }

    fn auth(&self, request: &String) -> Result<String> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials);
        }
        let mut signed_key = Hmac::<Sha256>::new_from_slice(self.secret.clone().unwrap().as_bytes())?;
        signed_key.update(request.as_bytes());
        Ok(hex::encode(signed_key.finalize().into_bytes()))
    }

    fn auth_map(&self, params: &HashMap<&str, &str>) -> Result<String> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials);
        }
        let params = params.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("&");
        Ok(self.auth(&params)?)
    }
}

#[async_trait]
impl Exchange for BinanceMargin {
    async fn connect(&mut self) -> ConnectResult<()> {
        self.load_markets().await?;
        self.exchange_base.connect().await?;
        Ok(())
    }
    async fn load_markets(&mut self) -> LoadMarketResult<Vec<Market>> {
        if self.exchange_base.markets.is_empty() {
            self.exchange_base.unifier.reset();
            let result: FetchMarketsResponse = self.exchange_base.http_client.get("/api/v3/exchangeInfo", EMPTY_QUERY).await?;
            let mut markets = vec![];
            for s in result.symbols {
                if let Ok(market) = (&s).into() {
                    self.exchange_base.unifier.insert_market_symbol_id(&market, &(s.symbol));
                    markets.push(market);
                }
            }
            self.exchange_base.markets = markets;
        }
        Ok(self.exchange_base.markets.clone())
    }

    async fn fetch_markets(&self) -> FetchMarketResult<Vec<Market>> {
        let result: FetchMarketsResponse = self.exchange_base.http_client.get("/api/v3/exchangeInfo", EMPTY_QUERY).await?;
        let mut markets = vec![];
        for s in result.symbols {
            let market: Result<Market> = (&s).into();
            let market = market?;
            markets.push(market);
        }
        Ok(markets)
    }

    async fn watch_order_book(&self, markets: &Vec<Market>) -> WatchResult<Receiver<OrderBookResult<OrderBook>>> {
        if !self.exchange_base.is_connected {
            return Err(WatchError::NotConnected);
        }

        if markets.len() == 0 {
            return Ok(self.exchange_base.order_book_stream_rx.clone())
        }

        let tx = self.exchange_base.ws_client.sender()
            .ok_or(Error::WebsocketError("no sender".into()))?;

        let mut symbol_ids: Vec<String> = Vec::new();
        for m in markets {
            match self.exchange_base.unifier.get_symbol_id(m) {
                Some(symbol_id) => symbol_ids.push(symbol_id),
                None => return Err(WatchError::SymbolNotFound(format!("{:?}", m))),
            }
        }
        let params = symbol_ids.iter()
            .map(|s| format!("\"{}@depth@100ms\"", s.to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");

        let stream_name = format!("{{\"method\": \"SUBSCRIBE\", \"params\": [{params}], \"id\": 1}}");
        tx.send_async(stream_name).await?;

        // todo check subscription result
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

        for m in markets.iter() {
            let symbol = self.exchange_base.unifier.get_symbol_id(m).ok_or(Error::SymbolNotFound(m.to_string()))?;
            let resp: GetOrderBookSnapshotResponse = self.exchange_base.http_client.get("/api/v3/depth", Some(&[("symbol", symbol), ("limit", "100".to_string())])).await?;
            let order_book = OrderBook::new(
                resp.bids.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>().map_err(|_| WatchError::UnknownError(format!("{:?}", resp)))?,
                resp.asks.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>().map_err(|_| WatchError::UnknownError(format!("{:?}", resp)))?,
                m.clone(),
                None,
                Some(resp.last_update_id),
            );
            self.exchange_base.order_book_synchronizer.read().unwrap().snapshot(m.clone(), order_book)?;
        }

        Ok(self.exchange_base.order_book_stream_rx.clone())
    }

    async fn create_order(&self, request: Order) -> CreateOrderResult<Order> {
        if request.price.is_none() && request.order_type == OrderType::Limit {
            return Err(Error::InvalidPrice("price is required for limit orders".into()).into());
        }
        let symbol_id = self.exchange_base.unifier.get_symbol_id(&request.market).ok_or(Error::SymbolNotFound(format!("{}", request.market)))?;
        let timestamp = Utc::now().timestamp_millis();

        let amount = request.amount.to_string();
        let timestamp = timestamp.to_string();
        let price = request.price.map_or("".to_string(), |price| price.to_string());

        let mut params: HashMap<&str, &str> = HashMap::new();
        params.insert("symbol", symbol_id.as_str());
        params.insert("side", util::get_exchange_order_side(&request.side.ok_or(Error::MissingField("side".into()))?));
        params.insert("type", util::get_exchange_order_type(&request.order_type)?);
        params.insert("quantity", amount.as_str());
        params.insert("timeInForce", util::get_exchange_time_in_force(&request.time_in_force.unwrap_or(TimeInForce::GTC)));
        params.insert("recvWindow", "5000");
        params.insert("timestamp", timestamp.as_str());
        params.insert("sideEffectType", util::get_exchange_margin_type(&request.margin_type));
        if price != "" {
            params.insert("price", price.as_str());
        }

        let signature = self.auth_map(&params)?;
        params.insert("signature", signature.as_str());
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let response: CreateOrderResponse = self.exchange_base.http_client.post("/sapi/v1/margin/order", Some(headers), Some(&params), EMPTY_BODY).await?;
        let mut order: Order = response.try_into()?;
        order.market = request.market;
        order.order_type = request.order_type;
        Ok(order)
    }
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetOrderBookSnapshotResponse {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: i64,
    pub bids: Vec<Vec<String>>,
    pub asks: Vec<Vec<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderResponse {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: i64,
    #[serde(rename = "clientOrderId")]
    pub client_order_id: String,
    #[serde(rename = "transactTime")]
    pub transaction_time: i64,
    pub price: String,
    #[serde(rename = "origQty")]
    pub original_quantity: String,
    #[serde(rename = "executedQty")]
    pub executed_quantity: String,
    #[serde(rename = "cummulativeQuoteQty")]
    pub cumulative_quote_quantity: String,
    pub status: String,
    #[serde(rename = "timeInForce")]
    pub time_in_force: String,
    #[serde(rename = "type")]
    pub order_type: String,
    #[serde(rename = "isIsolated")]
    pub is_isolated: bool,
    pub side: String,
    #[serde(rename = "selfTradePreventionMode")]
    pub self_trade_prevention_mode: String,
}

impl TryFrom<CreateOrderResponse> for Order {
    type Error = Error;

    fn try_from(resp: CreateOrderResponse) -> std::result::Result<Self, Self::Error> {
        let order_status = util::get_unified_order_status(&resp.status)?;
        let amount = resp.original_quantity.parse()?;
        let remaining = match order_status {
            OrderStatus::Open => Some(amount),
            _ => None,
        };
        Ok(Order {
            id: Some(resp.order_id.to_string()),
            client_order_id: Some(resp.client_order_id),
            timestamp: resp.transaction_time,
            status: order_status,
            time_in_force: Some(util::get_unified_time_in_force(&resp.time_in_force)?),
            side: Some(util::get_unified_order_side(&resp.side)?),
            price: Some(resp.price.parse()?),
            amount: resp.original_quantity.parse()?,
            remaining,
            ..Default::default()
        })
    }
}


#[derive(Serialize, Deserialize)]
struct WatchCommonResponse {
    result: Option<String>,
    id: Option<i64>,
    #[serde(rename = "e")]
    event_type: Option<String>,
}

impl TryFrom<Vec<u8>> for WatchCommonResponse {
    type Error = Error;

    fn try_from(message: Vec<u8>) -> Result<Self> {
        serde_json::from_slice(&message).map_err(|e| Error::WebsocketError(e.to_string()))
    }
}


#[derive(Serialize, Deserialize)]
struct WatchDiffOrderBookResponse {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: i64,
    #[serde(rename = "u")]
    pub final_update_id: i64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}


impl From<Vec<u8>> for WatchDiffOrderBookResponse {
    fn from(message: Vec<u8>) -> Self {
        serde_json::from_slice(&message).unwrap()
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchMarketsResponse {
    pub timezone: String,
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    #[serde(rename = "rateLimits")]
    pub rate_limits: Vec<RateLimit>,
    #[serde(rename = "exchangeFilters")]
    pub exchange_filters: Vec<Filter>,
    pub symbols: Vec<Symbol>,
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Symbol {
    pub symbol: String,
    pub status: Option<String>,
    #[serde(rename = "baseAsset")]
    pub base_asset: Option<String>,
    #[serde(rename = "baseAssetPrecision")]
    pub base_asset_precision: isize,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: Option<String>,
    #[serde(rename = "quoteAssetPrecision")]
    pub quote_asset_precision: i64,
    #[serde(rename = "orderTypes")]
    pub order_types: Vec<String>,
    #[serde(rename = "icebergAllowed")]
    pub iceberg_allowed: bool,
    #[serde(rename = "ocoAllowed")]
    pub oco_allowed: bool,
    #[serde(rename = "quoteOrderQtyMarketAllowed")]
    pub quote_order_qty_market_allowed: bool,
    #[serde(rename = "allowTrailingStop")]
    pub allow_trailing_stop: bool,
    #[serde(rename = "cancelReplaceAllowed")]
    pub cancel_replace_allowed: bool,
    #[serde(rename = "isSpotTradingAllowed")]
    pub is_spot_trading_allowed: bool,
    #[serde(rename = "isMarginTradingAllowed")]
    pub is_margin_trading_allowed: bool,
    pub filters: Option<Vec<Filter>>,
    pub permissions: Vec<String>,
    #[serde(rename = "defaultSelfTradePreventionMode")]
    pub default_self_trade_prevention_mode: String,
    #[serde(rename = "allowedSelfTradePreventionModes")]
    pub allowed_self_trade_prevention_modes: Vec<String>,
}

impl Into<Result<Market>> for &Symbol {
    fn into(self) -> Result<Market> {
        let base_id = self.base_asset.clone().ok_or_else(|| Error::MissingField("base_asset".into()))?;
        let quote_id = self.quote_asset.clone().ok_or_else(|| Error::MissingField("quote_asset".into()))?;

        let base = util::to_unified_asset(&base_id);
        let quote = util::to_unified_asset(&quote_id);

        let market_type = MarketType::Margin;

        let active = util::is_active(self.status.clone());

        if !self.permissions.contains(&("MARGIN".to_string())) {
            return Err(Error::InvalidMarket);
        }

        let mut limit = MarketLimit {
            amount: None,
            price: None,
            cost: None,
            leverage: None,
        };

        let mut precision = Precision {
            amount: None,
            price: Some(self.base_asset_precision),
            cost: None,
        };

        if let Some(filters) = &self.filters {
            for filter in filters.iter() {
                match filter.filter_type.as_str() {
                    "PRICE_FILTER" => {
                        let min = filter.min_price.as_ref().ok_or_else(|| Error::MissingField("min_price".into()))?.parse::<f64>()?;
                        let max = filter.max_price.as_ref().ok_or_else(|| Error::MissingField("max_price".into()))?.parse::<f64>()?;
                        limit.price = Some(Range { min, max });
                    }
                    "LOT_SIZE" => {
                        let min = filter.min_qty.as_ref().ok_or_else(|| Error::MissingField("min_qty".into()))?.parse::<f64>()?;
                        let max = filter.max_qty.as_ref().ok_or_else(|| Error::MissingField("max_qty".into()))?.parse::<f64>()?;
                        limit.amount = Some(Range { min, max });
                        let step_size = filter.step_size.as_ref().ok_or_else(|| Error::MissingField("step_size".into()))?;
                        precision.amount = Some(into_precision(step_size.clone())?);
                    }
                    "MIN_NOTIONAL" => {
                        let min = filter.min_notional.as_ref().ok_or_else(|| Error::MissingField("min_notional".into()))?.parse::<f64>()?;
                        let cost = limit.cost.map_or_else(|| Range { min, max: f64::MAX }, |r| Range { min, max: r.max });
                        limit.cost = Some(cost);
                    }
                    "NOTIONAL" => {
                        let min = filter.min_notional.as_ref().ok_or_else(|| Error::MissingField("min_notional".into()))?.parse::<f64>()?;
                        let max = filter.max_notional.as_ref().ok_or_else(|| Error::MissingField("max_notional".into()))?.parse::<f64>()?;
                        limit.cost = Some(Range { min, max });
                    }
                    // "MARKET_LOT_SIZE" => {},
                    // "MAX_NUM_ORDERS" => {},
                    // "MAX_NUM_ALGO_ORDERS" => {},
                    // "PERCENT_PRICE" => {},
                    _ => {}
                }
            }
        }
        Ok(Market {
            base,
            quote,
            active,
            market_type,
            precision: Some(precision),
            limit: Some(limit),
            ..Default::default()
        })
    }
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filter {
    pub filter_type: String,
    pub max_price: Option<String>,
    // PRICE_FILTER
    pub min_price: Option<String>,
    // PRICE_FILTER
    pub tick_size: Option<String>,
    // PRICE_FILTER
    pub multiplier_up: Option<String>,
    // PERCENT_PRICE
    pub multiplier_down: Option<String>,
    // PERCENT_PRICE
    pub avg_price_mins: Option<i64>,
    // PERCENT_PRICE, PERCENT_PRICE_BY_SIDE, MIN_NOTIONAL, NOTIONAL
    pub bid_multiplier_up: Option<String>,
    // PERCENT_PRICE_BY_SIDE
    pub bid_multiplier_down: Option<String>,
    // PERCENT_PRICE_BY_SIDE
    pub ask_multiplier_up: Option<String>,
    // PERCENT_PRICE_BY_SIDE
    pub ask_multiplier_down: Option<String>,
    // PERCENT_PRICE_BY_SIDE
    pub max_qty: Option<String>,
    // LOT_SIZE, MARKET_LOT_SIZE
    pub min_qty: Option<String>,
    // LOT_SIZE, MARKET_LOT_SIZE
    pub step_size: Option<String>,
    // LOT_SIZE
    pub min_notional: Option<String>,
    // MIN_NOTIONAL, NOTIONAL
    pub apply_to_market: Option<bool>,
    // MIN_NOTIONAL
    pub apply_min_to_market: Option<bool>,
    // NOTIONAL
    pub max_notional: Option<String>,
    // NOTIONAL
    pub apply_max_to_market: Option<bool>,
    // NOTIONAL
    pub limit: Option<i64>,
    // ICEBERG_PARTS
    pub max_num_orders: Option<i64>,
    // MAX_NUM_ORDERS, EXCHANGE_MAX_NUM_ORDERS
    pub max_num_algo_orders: Option<i64>,
    // MAX_NUM_ALGO_ORDERS, EXCHANGE_MAX_NUM_ALGO_ORDERS
    pub max_num_iceberg_orders: Option<i64>,
    // MAX_NUM_ICEBERG_ORDERS, EXCHANGE_MAX_NUM_ICEBERG_ORDERS
    pub max_position: Option<String>,
    // MAX_POSITION
    pub min_trailing_above_delta: Option<f64>,
    // TRAILING_DELTA
    pub max_trailing_above_delta: Option<f64>,
    // TRAILING_DELTA
    pub min_trailing_below_delta: Option<f64>,
    // TRAILING_DELTA
    pub max_trailing_below_delta: Option<f64>, // TRAILING_DELTA
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateLimit {
    pub interval: String,
    pub interval_num: i64,
    pub limit: i64,
    pub rate_limit_type: String,
}