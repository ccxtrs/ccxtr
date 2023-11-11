use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::client::EMPTY_BODY;
use crate::error::*;
use crate::exchange::*;
use crate::util::{into_precision, parse_float64};
use crate::util::channel::Receiver;

use super::util;

pub struct Binance {
    exchange_base: ExchangeBase,
    api_key: Option<String>,
    secret: Option<String>,
}

impl Binance {
    pub fn new(props: Properties) -> CommonResult<Self> {
        let base_props = BasePropertiesBuilder::default()
            .host(props.host.or(Some("https://api.binance.com".to_string())))
            .port(props.port.or(Some(443)))
            .ws_endpoint(props.ws_endpoint.or(Some("wss://stream.binance.com:9443/ws".to_string())))
            .error_parser(Some(|message| {
                match serde_json::from_str::<ErrorResponse>(&message) {
                    Ok(error) => {
                        match error.code {
                            -3045 => Error::InsufficientMargin(error.msg), // The system doesn't have enough asset now.
                            -1013 => Error::InvalidAmount(error.msg), // Invalid amount
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
                let common_message = WatchCommonResponse::try_from(message.to_vec())?;
                if common_message.code.is_some() && common_message.msg.is_some() { // error message
                    return Err(Error::StreamError(format!("code={}, msg={}", common_message.code.unwrap(), common_message.msg.unwrap())))?;
                }

                if common_message.id.is_some() { // subscription result
                    let id = common_message.id.ok_or(Error::InvalidResponse("id is not found".into()))?;
                    return Ok(StreamItem::Subscribed(id))
                }

                // best bid and ask stream
                if let (Some(order_book_update_id), Some(symbol), Some(bid_price), Some(bid_quantity), Some(ask_price), Some(ask_quantity)) = (common_message.order_book_update_id, common_message.symbol, common_message.bid_price, common_message.bid_quantity, common_message.ask_price, common_message.ask_quantity) {
                    let market = unifier.get_market(&symbol);
                    if market.is_none() {
                        return Ok(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Unknown market. symbol={}", symbol), None,
                        ))))
                    }
                    let market = market.unwrap();
                    let bid_price = bid_price.parse::<f64>();
                    if bid_price.is_err() {
                        return Ok(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Invalid bid price. symbol={}, price={}", symbol, bid_price.unwrap_err()), None,
                        ))))
                    }
                    let bid_price = bid_price.unwrap();
                    let bid_amount = bid_quantity.parse::<f64>();
                    if bid_amount.is_err() {
                        return Ok(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Invalid bid amount. symbol={}, amount={}", symbol, bid_amount.unwrap_err()), None,
                        ))))
                    }
                    let bid_amount = bid_amount.unwrap();
                    let ask_price = ask_price.parse::<f64>();
                    if ask_price.is_err() {
                        return Ok(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Invalid ask price. symbol={}, price={}", symbol, ask_price.unwrap_err()), None,
                        ))))
                    }
                    let ask_price = ask_price.unwrap();
                    let ask_amount = ask_quantity.parse::<f64>();
                    if ask_amount.is_err() {
                        return Ok(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                            format!("Invalid ask amount. symbol={}, amount={}", symbol, ask_amount.unwrap_err()), None,
                        ))))
                    }
                    let ask_amount = ask_amount.unwrap();
                    let book = OrderBook::new(vec![(bid_price, bid_amount).into()], vec![(ask_price, ask_amount).into()], market, None, Some(order_book_update_id));
                    return Ok(StreamItem::OrderBook(Ok(book)))
                }


                return match common_message.event_type {
                    Some(event_type) if event_type == "depthUpdate" => {
                        // diff order book
                        Ok(StreamItem::OrderBook(Err(OrderBookError::NotImplemented)))
                    }
                    _ => {
                        let message = String::from_utf8_lossy(&message);
                        Ok(StreamItem::Unknown(message.to_string()))
                    },
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
impl Exchange for Binance {
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

    async fn fetch_tickers(&self, params: FetchTickersParams) -> FetchTickersResult<Vec<Ticker>> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }

        let chunk_size = params.chunk_size.unwrap_or(20);

        let queries: Vec<Option<Vec<(&str, String)>>> = match params.markets {
            Some(markets) => {
                markets.chunks(chunk_size)
                    .map(|markets| {
                        let s = markets.iter()
                            .map(|m| self.exchange_base.unifier.get_symbol_id(&m))
                            .filter(|s| s.is_some())
                            .map(|s| format!("\"{}\"", s.unwrap()))
                            .collect::<Vec<String>>()
                            .join(",");
                        Some(vec![("symbols", format!("[{}]", s))])
                    })
                    .collect()
            }
            None => vec![None]
        };

        let mut tickers = vec![];
        for query in queries {
            let result: Vec<FetchTickersResponse> = self.exchange_base.http_client.get("/api/v3/ticker/24hr", None, query.as_ref()).await?;
            for item in result {
                let market = self.exchange_base.unifier.get_market(&item.symbol);
                if market.is_none() {
                    continue;
                }
                let market = market.unwrap();
                let timestamp = item.close_time;

                let last = item.last_price.parse::<f64>()?;
                let open = item.open_price.parse::<f64>()?;
                tickers.push(Ticker {
                    ask: Some(item.ask_price.parse::<f64>()?),
                    ask_volume: item.ask_qty.parse::<f64>()?,
                    bid: Some(item.bid_price.parse::<f64>()?),
                    bid_volume: item.bid_qty.parse::<f64>()?,
                    average: (last + open) / 2.0,
                    change: item.price_change.parse::<f64>()?,
                    close: last,
                    high: item.high_price.parse::<f64>()?,
                    last,
                    low: item.low_price.parse::<f64>()?,
                    open,
                    percentage: item.price_change_percent.parse::<f64>()?,
                    previous_close: Some(item.prev_close_price.parse::<f64>()?),
                    base_volume: item.volume.parse::<f64>()?,
                    quote_volume: item.quote_volume.parse::<f64>()?,
                    market,
                    timestamp,
                    vwap: item.weighted_avg_price.parse::<f64>()?,
                });
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        Ok(tickers)
    }

    async fn watch_order_book(&self, params: WatchOrderBookParams) -> WatchOrderBookResult<Receiver> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }

        let markets = &params.markets;

        if markets.len() == 0 {
            return Err(Error::MissingMarkets.into());
        }


        let mut symbol_ids: Vec<String> = Vec::new();
        for m in markets {
            match self.exchange_base.unifier.get_symbol_id(&m) {
                Some(symbol_id) => symbol_ids.push(symbol_id),
                None => return Err(WatchError::SymbolNotFound(format!("{:?}", m))),
            }
        }

        let mut clients = vec![];
        for symbol_ids in symbol_ids.chunks(100) {
            let params = symbol_ids.iter()
                    .map(|s| format!("\"{}@bookTicker\"", s.to_lowercase()))
                    .collect::<Vec<String>>()
                    .join(",");
                let stream_name = format!("{{\"method\": \"SUBSCRIBE\", \"params\": [{params}], \"id\": 1}}");
                let mut ws_client = WsClient::new(self.exchange_base.ws_endpoint.as_ref().unwrap().as_str(), self.exchange_base.stream_parser, self.exchange_base.unifier.clone());
                let _ = ws_client.send(stream_name).await?;
                clients.push(ws_client);
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }
        Ok(Receiver::new(clients))
    }


    async fn fetch_balance(&self, params: FetchBalanceParams) -> FetchBalanceResult<Balance> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials)?;
        }
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized)?;
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

    async fn create_order(&self, params: CreateOrderParams) -> CreateOrderResult<Order> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials)?;
        }

        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized)?;
        }

        let order_type = params.order_type.unwrap_or_default();
        if params.price.is_none() && order_type == OrderType::Limit {
            return Err(Error::InvalidPrice("price is required for limit orders".into()).into());
        }

        let symbol_id = self.exchange_base.unifier.get_symbol_id(&params.market).ok_or(Error::SymbolNotFound(format!("{}", params.market)))?;
        let timestamp = Utc::now().timestamp_millis();

        let amount = params.amount.to_string();
        let timestamp = timestamp.to_string();

        let side_effect_type = match (params.margin_mode, params.reduce_only) {
            (Some(_), true) => "REDUCE_ONLY",
            (Some(_), false) => "MARGIN_BUY",
            (None, _) => "NO_SIDE_EFFECT",
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
            ("recvWindow", "5000"),
            ("timestamp", timestamp.as_str()),
            ("sideEffectType", side_effect_type),
        ];

        if order_type != OrderType::Market {
            queries.push(("timeInForce", util::get_exchange_time_in_force(&params.time_in_force.unwrap_or(TimeInForce::GTC))));
        }

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
    code: Option<i64>,
    msg: Option<String>,
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
        serde_json::from_slice(&message).map_err(|e| {
            let message = String::from_utf8_lossy(message.as_slice());
            Error::DeserializeJsonBody(format!("Failed to deserialize json body. message={:?}, error={:?}", message, e))
        })
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
struct FetchTickersResponse {
    pub symbol: String,
    #[serde(rename = "priceChange")]
    pub price_change: String,
    #[serde(rename = "priceChangePercent")]
    pub price_change_percent: String,
    #[serde(rename = "weightedAvgPrice")]
    pub weighted_avg_price: String,
    #[serde(rename = "prevClosePrice")]
    pub prev_close_price: String,
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "lastQty")]
    pub last_qty: String,
    #[serde(rename = "bidPrice")]
    pub bid_price: String,
    #[serde(rename = "bidQty")]
    pub bid_qty: String,
    #[serde(rename = "askPrice")]
    pub ask_price: String,
    #[serde(rename = "askQty")]
    pub ask_qty: String,
    #[serde(rename = "openPrice")]
    pub open_price: String,
    #[serde(rename = "highPrice")]
    pub high_price: String,
    #[serde(rename = "lowPrice")]
    pub low_price: String,
    pub volume: String,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: String,
    #[serde(rename = "openTime")]
    pub open_time: i64,
    #[serde(rename = "closeTime")]
    pub close_time: i64,
    #[serde(rename = "firstId")]
    pub first_id: i64,
    #[serde(rename = "lastId")]
    pub last_id: i64,
    pub count: i64,
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

        let market_type = if self.is_margin_trading_allowed {
            MarketType::Margin
        } else {
            MarketType::Spot
        };

        let active = util::is_active(self.status.clone());

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
                        let tick_size = filter.tick_size.as_ref().ok_or_else(|| Error::MissingField("tick_size".into()))?;
                        precision.price = Some(into_precision(tick_size.clone())?);
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
    use crate::{Binance, Exchange, FetchTickersParamsBuilder, PropertiesBuilder};
    use crate::exchange::params::FetchBalanceParamsBuilder;
    use crate::model::MarginMode;

    #[tokio::test]
    async fn test_fetch_balance() {
        // get os environment variables
        let api_key = std::env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY is not set");
        let secret = std::env::var("BINANCE_SECRET").expect("BINANCE_SECRET is not set");

        let props = PropertiesBuilder::default()
            .api_key(Some(api_key))
            .secret(Some(secret))
            .host(Some("https://testnet.binance.vision".to_string()))
            .ws_endpoint(Some("wss://testnet.binance.vision/ws".to_string()))
            .build().expect("failed to create properties");
        let mut exchange = Binance::new(props).expect("failed to create exchange");
        let markets = exchange.load_markets().await.expect("failed to load markets");
        assert!(markets.len() > 0);
        let params = FetchBalanceParamsBuilder::default().margin_mode(Some(MarginMode::Cross)).build().unwrap();
        let balance = exchange.fetch_balance(params).await;
        let balance = balance.expect("failed to fetch balance");
        balance.items.iter().for_each(|item| {
            if item.currency != "USDT" && item.currency != "BTC" {
                return;
            }
            println!("{:?}", item);
        });
    }

    #[tokio::test]
    async fn test_fetch_tickers() {
        let mut exchange = Binance::new(PropertiesBuilder::default().build().unwrap()).unwrap();
        let markets = exchange.load_markets().await.unwrap();
        let target_market = markets.into_iter().find(|m| m.base == "BTC" && m.quote == "USDT").unwrap();
        let params = FetchTickersParamsBuilder::default().markets(Some(vec![target_market])).build().unwrap();
        let tickers = exchange.fetch_tickers(params).await;
        tickers.unwrap().iter().for_each(|ticker| {
            println!("{:?}", ticker);
        });
    }
}
