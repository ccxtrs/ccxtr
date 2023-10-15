use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::client::EMPTY_BODY;
use crate::error::*;
use crate::exchange::*;
use crate::util::channel::Receiver;
use crate::util::{into_precision, parse_float64};

use super::util;


pub struct BinanceMargin {
    exchange_base: ExchangeBase,
    api_key: Option<String>,
    secret: Option<String>,
}

impl BinanceMargin {
    pub fn new(props: &Properties) -> CommonResult<Self> {
        let base_props = BasePropertiesBuilder::default()
            .host(props.host.clone().or(Some("https://api.binance.com".to_string())))
            .port(props.port.or(Some(443)))
            .ws_endpoint(Some("wss://stream.binance.com:9443/ws".to_string()))
            .error_parser(Some(|message| {
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
            }))
            .stream_parser(Some(|message, unifier| {
                let common_message = WatchCommonResponse::try_from(message.clone()).ok()?;
                if common_message.result.is_some() { // subscription result
                    return None;
                }

                // best bid and ask stream
                if let (Some(order_book_update_id), Some(symbol), Some(bid_price), Some(bid_quantity), Some(ask_price), Some(ask_quantity)) = (common_message.order_book_update_id, common_message.symbol, common_message.bid_price, common_message.bid_quantity, common_message.ask_price, common_message.ask_quantity) {
                    let market = unifier.get_market(&symbol);
                    if market.is_none() {
                        return Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Unknown market. symbol={}", symbol), None,
                        ))));
                    }
                    let market = market.unwrap();
                    let bid_price = bid_price.parse::<f64>().unwrap();
                    let bid_quantity = bid_quantity.parse::<f64>().unwrap();
                    let ask_price = ask_price.parse::<f64>().unwrap();
                    let ask_quantity = ask_quantity.parse::<f64>().unwrap();
                    let book = OrderBook::new(vec![(bid_price, bid_quantity).into()], vec![(ask_price, ask_quantity).into()], market, None, Some(order_book_update_id));
                    return Some(StreamItem::OrderBook(Ok(book)));
                }


                return match common_message.event_type {
                    Some(event_type) if event_type == "depthUpdate" => {
                        // diff order book
                        Some(StreamItem::OrderBook(Err(OrderBookError::NotImplemented)))
                    }
                    _ => None,
                };
            }))
            .channel_capacity(props.channel_capacity)
            .build()?;

        Ok(Self {
            exchange_base: ExchangeBase::new(&base_props)?,
            api_key: props.api_key.clone(),
            secret: props.secret.clone(),
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

    fn auth_map(&self, params: Option<&Vec<(&str, &str)>>) -> Result<String> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials);
        }
        match params {
            Some(params) => {
                let params = params.iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<String>>()
                    .join("&");
                Ok(self.auth(&params)?)
            }
            None => Ok(self.auth(&"".to_string())?),
        }
    }
}

#[async_trait]
impl Exchange for BinanceMargin {
    async fn load_markets(&mut self) -> LoadMarketResult<Vec<Market>> {
        if self.exchange_base.markets.is_empty() {
            self.exchange_base.unifier.reset();
            let result = self.exchange_base.http_client.get::<(), FetchMarketsResponse>("/api/v3/exchangeInfo", None, None).await?;
            let mut markets = vec![];
            for s in result.symbols {
                if let Ok(market) = (&s).into() {
                    self.exchange_base.unifier.insert_market_symbol_id(&market, &(s.symbol));
                    markets.push(market);
                }
            }
            self.exchange_base.markets = markets;
            self.exchange_base.connect().await?;
        }
        Ok(self.exchange_base.markets.clone())
    }

    async fn fetch_markets(&self) -> FetchMarketResult<Vec<Market>> {
        let result = self.exchange_base.http_client.get::<(), FetchMarketsResponse>("/api/v3/exchangeInfo", None, None).await?;
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
            return Ok(self.exchange_base.order_book_stream_rx.clone());
        }

        let sender = self.exchange_base.ws_client.sender()
            .ok_or(Error::WebsocketError("no sender".into()))?;

        let mut symbol_ids: Vec<String> = Vec::new();
        for m in markets {
            match self.exchange_base.unifier.get_symbol_id(&m) {
                Some(symbol_id) => symbol_ids.push(symbol_id),
                None => return Err(WatchError::SymbolNotFound(format!("{:?}", m))),
            }
        }
        let params = symbol_ids.iter()
            .map(|s| format!("\"{}@bookTicker\"", s.to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");

        let stream_name = format!("{{\"method\": \"SUBSCRIBE\", \"params\": [{params}], \"id\": 1}}");
        sender.send_async(stream_name).await?;

        Ok(self.exchange_base.order_book_stream_rx.clone())
    }

    async fn fetch_balance(&self, params: &FetchBalanceParams) -> FetchBalanceResult<Balance> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials)?;
        }

        let ts = Utc::now().timestamp_millis().to_string();
        let mut query = vec![];
        query.push(("timestamp", ts.as_str()));
        let signature = self.auth_map(Some(&query))?;
        query.push(("signature", signature.as_str()));
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];

        match params.margin_mode {
            Some(MarginMode::Cross) => {
                let resp: FetchAccountResponse = self.exchange_base.http_client.get("/sapi/v1/margin/account", Some(headers), Some(&query)).await?;
                let mut items = vec![];
                for asset in resp.user_assets {
                    let used = parse_float64(&asset.locked)?;
                    let free = parse_float64(&asset.free)?;
                    items.push(BalanceItem {
                        currency: util::to_unified_asset(asset.asset.as_str()),
                        market: None,
                        used,
                        free,
                        debt: parse_float64(&asset.interest)? + parse_float64(&asset.borrowed)?,
                        total: free + used,
                    });
                }
                Ok(Balance {
                    timestamp: None,
                    items,
                })
            }
            Some(MarginMode::Isolated) => {
                let resp: FetchIsolatedAccountResponse = self.exchange_base.http_client.get("/sapi/v1/margin/isolated/account", Some(headers), Some(&query)).await?;
                let mut items = vec![];
                for symbol in resp.assets {
                    let base_used = parse_float64(&symbol.base_asset.locked)?;
                    let base_free = parse_float64(&symbol.base_asset.free)?;
                    items.push(BalanceItem {
                        currency: util::to_unified_asset(symbol.base_asset.asset.as_str()),
                        market: self.exchange_base.unifier.get_market(&symbol.symbol),
                        used: base_used,
                        free: base_free,
                        debt: parse_float64(&symbol.base_asset.interest)? + parse_float64(&symbol.base_asset.borrowed)?,
                        total: base_free + base_used,
                    });
                    let quote_used = parse_float64(&symbol.quote_asset.locked)?;
                    let quote_free = parse_float64(&symbol.quote_asset.free)?;
                    items.push(BalanceItem {
                        currency: util::to_unified_asset(symbol.quote_asset.asset.as_str()),
                        market: self.exchange_base.unifier.get_market(&symbol.symbol),
                        used: quote_used,
                        free: quote_free,
                        debt: parse_float64(&symbol.quote_asset.interest)? + parse_float64(&symbol.quote_asset.borrowed)?,
                        total: quote_free + quote_used,
                    });
                }
                Ok(Balance {
                    timestamp: None,
                    items,
                })
            }
            None => Err(Error::InvalidParameters("margin_mode is required".into()))?,
        }
    }

    async fn create_order(&self, params: &CreateOrderParams) -> CreateOrderResult<Order> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials)?;
        }
        let order_type = params.order_type.unwrap_or_default();
        if params.price.is_none() && order_type == OrderType::Limit {
            return Err(Error::InvalidPrice("price is required for limit orders".into()).into());
        }

        let symbol_id = self.exchange_base.unifier.get_symbol_id(&params.market).ok_or(Error::SymbolNotFound(format!("{}", params.market)))?;
        let timestamp = Utc::now().timestamp_millis();

        let amount = params.amount.to_string();
        let timestamp = timestamp.to_string();

        let side_effect_type = match params.reduce_only {
            true => "REDUCE_ONLY",
            false => "MARGIN_BUY",
        };

        let is_isolated = match matches!(params.margin_mode, Some(MarginMode::Isolated)) {
            true => "TRUE",
            false => "FALSE",
        };

        let mut queries = vec![
            ("symbol", symbol_id.as_str()),
            ("isIsolated", is_isolated),
            ("side", util::get_exchange_order_side(&params.order_side)),
            ("type", util::get_exchange_order_type(&order_type)?),
            ("quantity", amount.as_str()),
            ("timeInForce", util::get_exchange_time_in_force(&params.time_in_force.unwrap_or(TimeInForce::GTC))),
            ("recvWindow", "5000"),
            ("timestamp", timestamp.as_str()),
            ("sideEffectType", side_effect_type),
        ];

        let price = params.price.map(|p| p.to_string());
        if params.price.is_some() {
            queries.push(("price", price.as_ref().unwrap().as_str()));
        }

        let signature = self.auth_map(Some(&queries))?;
        queries.push(("signature", signature.as_str()));
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let response: CreateOrderResponse = self.exchange_base.http_client.post("/sapi/v1/margin/order", Some(headers), Some(&queries), EMPTY_BODY).await?;
        let mut order: Order = response.try_into()?;
        order.market = params.market.clone();
        order.order_type = order_type;
        Ok(order)
    }
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

    /// for best bid and ask stream
    #[serde(rename = "u")]
    order_book_update_id: Option<i64>,
    #[serde(rename = "s")]
    symbol: Option<String>,
    #[serde(rename = "b")]
    bid_price: Option<String>,
    #[serde(rename = "B")]
    bid_quantity: Option<String>,
    #[serde(rename = "a")]
    ask_price: Option<String>,
    #[serde(rename = "A")]
    ask_quantity: Option<String>,
}

impl TryFrom<Vec<u8>> for WatchCommonResponse {
    type Error = Error;

    fn try_from(message: Vec<u8>) -> Result<Self> {
        serde_json::from_slice(&message).map_err(|e| Error::WebsocketError(e.to_string()))
    }
}


#[derive(Serialize, Deserialize)]
struct WatchPartialOrderBookResponse {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: i64,
    pub bids: Vec<Vec<String>>,
    pub asks: Vec<Vec<String>>,
}

impl From<Vec<u8>> for WatchPartialOrderBookResponse {
    fn from(message: Vec<u8>) -> Self {
        serde_json::from_slice(&message).unwrap()
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
struct FetchIsolatedAccountAssetResponse {
    pub asset: String,
    #[serde(rename = "borrowEnabled")]
    pub borrow_enabled: bool,
    pub borrowed: String,
    pub free: String,
    pub interest: String,
    pub locked: String,
    #[serde(rename = "netAsset")]
    pub net_asset: String,
    #[serde(rename = "netAssetOfBtc")]
    pub net_asset_of_btc: String,
    #[serde(rename = "repayEnabled")]
    pub repay_enabled: bool,
    #[serde(rename = "totalAsset")]
    pub total_asset: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchIsolatedAccountSymbolResponse {
    #[serde(rename = "baseAsset")]
    pub base_asset: FetchIsolatedAccountAssetResponse,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: FetchIsolatedAccountAssetResponse,
    pub symbol: String,
    #[serde(rename = "isolatedCreated")]
    pub isolated_created: bool,
    pub enabled: bool,
    #[serde(rename = "marginLevel")]
    pub margin_level: String,
    #[serde(rename = "marginLevelStatus")]
    pub margin_level_status: String,
    #[serde(rename = "marginRatio")]
    pub margin_ratio: String,
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    #[serde(rename = "liquidatePrice")]
    pub liquidate_price: String,
    #[serde(rename = "liquidateRate")]
    pub liquidate_rate: String,
    #[serde(rename = "tradeEnabled")]
    pub trade_enabled: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchIsolatedAccountResponse {
    pub assets: Vec<FetchIsolatedAccountSymbolResponse>,
    #[serde(rename = "totalAssetOfBtc")]
    pub total_asset_of_btc: String,
    #[serde(rename = "totalLiabilityOfBtc")]
    pub total_liability_of_btc: String,
    #[serde(rename = "totalNetAssetOfBtc")]
    pub total_net_asset_of_btc: String,
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchAccountAssetResponse {
    pub asset: String,
    pub borrowed: String,
    pub free: String,
    pub interest: String,
    pub locked: String,
    #[serde(rename = "netAsset")]
    pub net_asset: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchAccountResponse {
    #[serde(rename = "borrowEnabled")]
    pub borrow_enabled: bool,
    #[serde(rename = "marginLevel")]
    pub margin_level: String,
    #[serde(rename = "totalAssetOfBtc")]
    pub total_asset_of_btc: String,
    #[serde(rename = "totalLiabilityOfBtc")]
    pub total_liability_of_btc: String,
    #[serde(rename = "totalNetAssetOfBtc")]
    pub total_net_asset_of_btc: String,
    #[serde(rename = "tradeEnabled")]
    pub trade_enabled: bool,
    #[serde(rename = "transferEnabled")]
    pub transfer_enabled: bool,
    #[serde(rename = "userAssets")]
    pub user_assets: Vec<FetchAccountAssetResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchMarketsResponse {
    pub timezone: String,
    #[serde(rename = "serverTime")]
    pub server_time: i64,
    #[serde(rename = "rateLimits")]
    pub rate_limits: Vec<FetchMarketRateLimitResponse>,
    #[serde(rename = "exchangeFilters")]
    pub exchange_filters: Vec<FetchMarketFilterResponse>,
    pub symbols: Vec<FetchMarketsSymbolResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchMarketsSymbolResponse {
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
    pub filters: Option<Vec<FetchMarketFilterResponse>>,
    pub permissions: Vec<String>,
    #[serde(rename = "defaultSelfTradePreventionMode")]
    pub default_self_trade_prevention_mode: String,
    #[serde(rename = "allowedSelfTradePreventionModes")]
    pub allowed_self_trade_prevention_modes: Vec<String>,
}

impl Into<Result<Market>> for &FetchMarketsSymbolResponse {
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
struct FetchMarketFilterResponse {
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
struct FetchMarketRateLimitResponse {
    pub interval: String,
    pub interval_num: i64,
    pub limit: i64,
    pub rate_limit_type: String,
}


#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    code: i64,
    msg: String,
}


#[cfg(test)]
mod test {
    use crate::{BinanceMargin, Exchange, PropertiesBuilder};
    use crate::exchange::params::FetchBalanceParamsBuilder;
    use crate::model::MarginMode;

    #[tokio::test]
    async fn test_fetch_balance() {
        // get os environment variables
        let api_key = std::env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY is not set");
        let secret = std::env::var("BINANCE_SECRET").expect("BINANCE_SECRET is not set");

        let props = PropertiesBuilder::default().api_key(Some(api_key)).secret(Some(secret)).build().expect("failed to create properties");
        let exchange = BinanceMargin::new(&props).expect("failed to create exchange");
        let params = FetchBalanceParamsBuilder::default().margin_mode(Some(MarginMode::Cross)).build().unwrap();
        let balance = exchange.fetch_balance(&params).await;
        let balance = balance.expect("failed to fetch balance");
        balance.items.iter().for_each(|item| {
            if item.currency != "USDT" && item.currency != "BTC" {
                return;
            }
            println!("{:?}", item);
        });
    }
}
