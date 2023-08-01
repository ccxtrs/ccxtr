use std::fmt::Debug;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use chrono::LocalResult::Single;
use futures::channel::mpsc::Receiver;
use futures::SinkExt;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{CommonResult, CreateOrderResult, exchange::{Exchange, Properties}, FetchMarketResult, model::{Market, MarketType}, OrderBookResult, PropertiesBuilder, WatchResult};
use crate::client::EMPTY_QUERY;
use crate::error::{CreateOrderError, Error, Result, LoadMarketResult, OrderBookError, WatchError};
use crate::exchange::{ExchangeBase, StreamItem};
use crate::exchange::binance::util;
use crate::model::{ContractType, Order, OrderBook, OrderBookUnit, OrderStatus, OrderType, TimeInForce};
use crate::model::{MarketLimit, Precision, Range};
use crate::util::into_precision;

pub struct BinanceUsdm {
    exchange_base: ExchangeBase,
    api_key: Option<String>,
    secret: Option<String>,
}


#[derive(Serialize, Deserialize)]
struct WatchOrderBookResponse {
    #[serde(rename = "e")]
    event_type: String,
    #[serde(rename = "E")]
    event_time: i64,
    #[serde(rename = "T")]
    transaction_time: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "U")]
    first_update_id: i64,
    #[serde(rename = "u")]
    final_update_id: i64,
    #[serde(rename = "b")]
    bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    asks: Vec<[String; 2]>,
    #[serde(rename = "pu")]
    previous_final_update_id: i64,
}


#[derive(Serialize, Deserialize)]
struct WatchCommonResponse {
    result: Option<String>,
    id: Option<i64>,
    #[serde(rename = "e")]
    event_type: Option<String>,
}

impl From<Vec<u8>> for WatchCommonResponse {
    fn from(message: Vec<u8>) -> Self {
        serde_json::from_slice(&message).unwrap()
    }
}

impl From<Vec<u8>> for WatchOrderBookResponse {
    fn from(message: Vec<u8>) -> Self {
        serde_json::from_slice(&message).unwrap()
    }
}

impl TryFrom<&[String; 2]> for OrderBookUnit {
    type Error = OrderBookError;

    fn try_from(value: &[String; 2]) -> OrderBookResult<Self> {
        Ok(OrderBookUnit {
            price: value[0].parse::<f64>()?,
            amount: value[1].parse::<f64>()?,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    code: i64,
    msg: String,
}

impl BinanceUsdm {
    pub fn new(props: Properties) -> CommonResult<Self> {
        let common_props = PropertiesBuilder::new()
            .host(props.host.unwrap_or_else(|| "https://fapi.binance.com".into()))
            .port(props.port.unwrap_or(443))
            .ws_endpoint("wss://fstream.binance.com/ws")
            .error_parser(|message| {
                let error: ErrorResponse = serde_json::from_str(&message).unwrap();
                match error.code {
                    -2019 => Error::HttpError("Margin is insufficient".to_string()),
                    -1013 => Error::HttpError("Invalid quantity".to_string()),
                    -1021 => Error::HttpError("Timestamp for this request is outside of the recvWindow".to_string()),
                    -1022 => Error::HttpError("Signature for this request is not valid".to_string()),
                    -1100 => Error::HttpError("Illegal characters found in a parameter".to_string()),
                    -1101 => Error::HttpError("Too many parameters sent for this endpoint".to_string()),
                    _ => Error::HttpError(error.msg),
                }
            })
            .stream_parser(|message, unifier| {
                let common_message = WatchCommonResponse::from(message.clone());
                if common_message.result.is_some() {
                    return None;
                }
                match common_message.event_type {
                    Some(event_type) if event_type == "depthUpdate" => {
                        let resp = WatchOrderBookResponse::from(message);
                        let market = unifier.get_market(&resp.symbol);
                        if market.is_none() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Unknown market {}", resp.symbol),
                            ))));
                        }
                        let bids = resp.bids.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if bids.is_err() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid bid {:?}", resp.bids),
                            ))));
                        }
                        let asks = resp.asks.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if asks.is_err() {
                            return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid ask {:?}", resp.asks),
                            ))));
                        }
                        let book = OrderBook::new(
                            bids.unwrap(),
                            asks.unwrap(),
                            market.unwrap(),
                            resp.event_time,
                            Utc.timestamp_millis_opt(resp.event_time).unwrap().to_rfc3339(),
                            None,
                        );
                        Some(StreamItem::OrderBook(Ok(book)))
                    }
                    _ => return None,
                }
            });

        Ok(Self {
            exchange_base: ExchangeBase::new(common_props.build())?,
            api_key: props.api_key,
            secret: props.secret,
        })
    }

    fn auth(&self, request: &String) -> Result<String> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials);
        }
        let mut signed_key = Hmac::<Sha256>::new_from_slice(self.secret.clone().unwrap().as_bytes())?;
        signed_key.update(request.as_bytes());
        Ok(hex::encode(signed_key.finalize().into_bytes()))
    }

    pub async fn connect(&mut self) -> CommonResult<()> {
        Ok(self.exchange_base.connect().await?)
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&mut self) -> LoadMarketResult<&Vec<Market>> {
        if self.exchange_base.markets.is_empty() {
            self.fetch_markets().await?;
        }
        Ok(&self.exchange_base.markets)
    }

    async fn fetch_markets(&mut self) -> FetchMarketResult<&Vec<Market>> {
        let result: FetchMarketsResponse = self.exchange_base.http_client.get("/fapi/v1/exchangeInfo", EMPTY_QUERY).await?;
        self.exchange_base.unifier.reset();
        let mut markets = vec![];
        for s in result.symbols {
            if s.symbol.is_none() {
                continue;
            }
            let market: Result<Market> = (&s).into();
            let market = market?;
            self.exchange_base.unifier.insert_market_symbol_id(&market, &(s.symbol.unwrap()));
            markets.push(market);
        }
        self.exchange_base.markets = markets;
        Ok(&self.exchange_base.markets)
    }

    async fn watch_order_book(&mut self, markets: &Vec<Market>) -> WatchResult<Receiver<OrderBookResult<OrderBook>>> {
        let mut sender = self.exchange_base.ws_client.sender()
            .ok_or(Error::WebsocketError("no sender".into()))?;

        let mut symbol_ids: Vec<String> = Vec::new();
        for m in markets {
            match self.exchange_base.unifier.get_symbol_id(&m) {
                Some(symbol_id) => symbol_ids.push(symbol_id),
                None => return Err(WatchError::SymbolNotFound(format!("{:?}", m))),
            }
        }
        let params = symbol_ids.iter()
            .map(|s| format!("\"{}@depth5@100ms\"", s.to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");

        let stream_name = format!("{{\"method\": \"SUBSCRIBE\", \"params\": [{params}], \"id\": 1}}");
        sender.send(stream_name).await?;

        Ok(self.exchange_base.order_book_stream.take()
            .ok_or(Error::WebsocketError("no receiver".into()))?)
    }

    async fn create_order(&self, request: Order) -> CreateOrderResult<Order> {
        if request.price.is_none() && request.order_type == OrderType::Limit {
            return Err(Error::InvalidPrice("price is required for limit orders".into()).into());
        }
        let symbol_id = self.exchange_base.unifier.get_symbol_id(&request.market).ok_or(Error::SymbolNotFound(format!("{}", request.market)))?;
        let timestamp = Utc::now().timestamp_millis();
        let mut params = format!("symbol={}&side={}&type={}&quantity={}&timeInForce={}&recvWindow=5000&timestamp={}",
                             symbol_id,
                             util::get_exchange_order_side(&request.side),
                             util::get_exchange_order_type(&request.order_type)?,
                             request.amount,
                             util::get_exchange_time_in_force(&request.time_in_force.unwrap_or(TimeInForce::GTC)),
                             timestamp);
        if request.price.is_some() {
            params = format!("{}&price={}", params, request.price.unwrap());
        }
        let signature = self.auth(&params)?;
        let params = format!("{}&signature={}", params, signature);
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let response: CreateOrderResponse = self.exchange_base.http_client.post("/fapi/v1/order", Some(headers), EMPTY_QUERY, Some(&params)).await?;
        let mut order: Order = response.try_into()?;
        order.market = request.market;
        order.order_type = request.order_type;
        Ok(order)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct CreateOrderResponse {
    client_order_id: String,
    cum_quote: String,
    executed_qty: String,
    order_id: i64,
    avg_price: String,
    orig_qty: String,
    price: String,
    reduce_only: bool,
    side: String,
    position_side: String,
    status: String,
    stop_price: String,
    close_position: bool,
    symbol: String,
    time_in_force: String,
    #[serde(rename = "type")]
    order_type: String,
    orig_type: String,
    activate_price: Option<String>,
    price_rate: Option<String>,
    update_time: i64,
    working_type: String,
    price_protect: bool,
}

impl TryFrom<CreateOrderResponse> for Order {
    type Error = Error;

    fn try_from(resp: CreateOrderResponse) -> std::result::Result<Self, Self::Error> {
        let timestamp = Utc.timestamp_millis_opt(resp.update_time).unwrap();
        let order_status = util::get_unified_order_status(&resp.status)?;
        let amount = resp.orig_qty.parse()?;
        let remaining = match order_status {
            OrderStatus::Open => Some(amount),
            _ => None,
        };
        Ok(Order {
            id: Some(resp.order_id.to_string()),
            client_order_id: Some(resp.client_order_id),
            datetime: timestamp.to_rfc3339(),
            timestamp: resp.update_time,
            status: order_status,
            time_in_force: Some(util::get_unified_time_in_force(&resp.time_in_force)?),
            side: util::get_unified_order_side(&resp.side)?,
            price: Some(resp.price.parse()?),
            average: Some(resp.avg_price.parse()?),
            amount: resp.orig_qty.parse()?,
            remaining,
            ..Default::default()
        })
    }
}


#[derive(Serialize)]
struct LoadMarketsRequest {}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct FetchMarketsResponse {
    pub exchange_filters: Option<Vec<String>>,
    pub rate_limits: Vec<RateLimit>,
    pub server_time: i64,
    pub assets: Option<Vec<Asset>>,
    pub symbols: Vec<Symbol>,
    pub timezone: String,
}

impl TryInto<Vec<Market>> for FetchMarketsResponse {
    type Error = Error;

    fn try_into(self) -> std::result::Result<Vec<Market>, Self::Error> {
        self.symbols.into_iter()
            .map(|s| (&s).into())
            .collect()
    }
}


impl Into<Result<Market>> for &Symbol {
    fn into(self) -> Result<Market> {
        let base_id = self.base_asset.clone().ok_or_else(|| Error::MissingField("base_asset".into()))?;
        let quote_id = self.quote_asset.clone().ok_or_else(|| Error::MissingField("quote_asset".into()))?;
        let settle_id = self.margin_asset.clone();


        let base = util::to_unified_asset(&base_id);
        let quote = util::to_unified_asset(&quote_id);
        let settle = settle_id.as_ref()
            .and_then(|s| Some(util::to_unified_asset(s)));

        let market_type = match self.contract_type {
            Some(ref s) if s == "PERPETUAL" => MarketType::Swap,
            Some(ref s) if s == "CURRENT_QUARTER" => MarketType::Futures,
            Some(ref s) if s == "NEXT_QUARTER" => MarketType::Futures,
            _ => MarketType::Unknown,
        };

        let delivery_date = self.delivery_date
            .and_then(|ts| match Utc.timestamp_millis_opt(ts) {
                Single(dt) => Some(dt),
                _ => None,
            });

        let active = util::is_active(self.status.clone());

        let mut limit = MarketLimit {
            amount: None,
            price: None,
            cost: None,
            leverage: None,
        };

        let mut precision = Precision {
            amount: self.quantity_precision,
            price: self.price_precision,
            cost: None,
        };

        for filter in self.filters.iter().flatten() {
            match filter.filter_type.as_str() {
                "PRICE_FILTER" => {
                    let start = filter.min_price.as_ref().ok_or_else(|| Error::MissingField("min_price".into()))?.parse::<f64>()?;
                    let end = filter.max_price.as_ref().ok_or_else(|| Error::MissingField("max_price".into()))?.parse::<f64>()?;
                    limit.price = Some(Range { start, end });
                    let tick_size = filter.tick_size.as_ref().ok_or_else(|| Error::MissingField("tick_size".into()))?;
                    precision.price = Some(into_precision(tick_size.clone())?);
                }
                "LOT_SIZE" => {
                    let start = filter.min_qty.as_ref().ok_or_else(|| Error::MissingField("min_qty".into()))?.parse::<f64>()?;
                    let end = filter.max_qty.as_ref().ok_or_else(|| Error::MissingField("max_qty".into()))?.parse::<f64>()?;
                    limit.amount = Some(Range { start, end });
                }
                "MIN_NOTIONAL" => {
                    let start = filter.notional.as_ref().ok_or_else(|| Error::MissingField("notional".into()))?.parse::<f64>()?;
                    limit.cost = Some(Range { start, end: f64::MAX });
                }
                // "MARKET_LOT_SIZE" => {},
                // "MAX_NUM_ORDERS" => {},
                // "MAX_NUM_ALGO_ORDERS" => {},
                // "PERCENT_PRICE" => {},
                _ => {}
            }
        }
        Ok(Market {
            base,
            quote,
            active,
            market_type,
            settle,
            contract_size: None,
            contract_type: Some(ContractType::Linear),
            expiry: self.delivery_date,
            expiry_datetime: delivery_date.and_then(|dt| Some(dt.format("%Y-%m-%d %H:%M:%S").to_string())).unwrap_or_else(|| "".into()),
            strike: None,
            option_type: None,
            fee: None,
            fee_currency: None,
            fee_currency_id: None,
            fee_side: None,
            precision: Some(precision),
            limit: Some(limit),
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct RateLimit {
    pub interval: String,
    pub interval_num: i64,
    pub limit: i64,
    pub rate_limit_type: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct Asset {
    pub asset: String,
    pub margin_available: bool,
    pub auto_asset_exchange: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct Symbol {
    pub symbol: Option<String>,
    pub pair: Option<String>,
    pub contract_type: Option<String>,
    pub delivery_date: Option<i64>,
    pub onboard_date: Option<i64>,
    pub status: Option<String>,
    pub maint_margin_percent: Option<String>,
    pub required_margin_percent: Option<String>,
    pub base_asset: Option<String>,
    pub quote_asset: Option<String>,
    pub margin_asset: Option<String>,
    pub price_precision: Option<isize>,
    pub quantity_precision: Option<isize>,
    pub base_asset_precision: Option<isize>,
    pub quote_precision: Option<isize>,
    pub underlying_type: Option<String>,
    pub underlying_sub_type: Option<Vec<String>>,
    pub settle_plan: Option<i64>,
    pub trigger_protect: Option<String>,
    pub filters: Option<Vec<Filter>>,
    pub order_type: Option<Vec<String>>,
    pub time_in_force: Option<Vec<String>>,
    pub liquidation_fee: Option<String>,
    pub market_take_bound: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct Filter {
    pub filter_type: String,
    pub max_price: Option<String>,
    pub min_price: Option<String>,
    pub tick_size: Option<String>,
    pub max_qty: Option<String>,
    pub min_qty: Option<String>,
    pub step_size: Option<String>,
    pub limit: Option<i64>,
    pub notional: Option<String>,
    pub multiplier_up: Option<String>,
    pub multiplier_down: Option<String>,
    pub multiplier_decimal: Option<String>,
}

