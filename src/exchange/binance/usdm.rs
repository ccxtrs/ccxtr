use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::client::EMPTY_QUERY;
use crate::error::*;
use crate::exchange::*;
use crate::util::{into_precision, parse_float64};
use crate::util::channel::Receiver;

use super::util;

pub struct BinanceUsdm {
    exchange_base: ExchangeBase,
    api_key: Option<String>,
    secret: Option<String>,
    leverage_brackets: Option<HashMap<Market, Vec<LeverageBracket>>>,
}


impl BinanceUsdm {
    pub fn new(props: Properties) -> CommonResult<Self> {
        let base_props = BasePropertiesBuilder::default()
            .host(props.host.or(Some("https://fapi.binance.com".to_string())))
            .port(props.port.or(Some(443)))
            .ws_endpoint(props.ws_endpoint.or(Some("wss://fstream.binance.com/ws".to_string())))
            .error_parser(Some(|message| {
                match serde_json::from_str::<ErrorResponse>(&message) {
                    Ok(error) => {
                        match error.code {
                            -2019 => Error::InsufficientMargin(error.msg), // Margin is insufficient
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
                    return Err(Error::InvalidResponse(format!("code={}, msg={}", common_message.code.unwrap(), common_message.msg.unwrap())))?;
                }
                if common_message.id.is_some() { // subscription response
                    let id = common_message.id.ok_or(Error::InvalidResponse("id is not found".into()))?;
                    return Ok(Some(StreamItem::Subscribed(id)));
                }
                match common_message.event_type {
                    Some(event_type) if event_type == "depthUpdate" => {
                        let resp = WatchOrderBookResponse::try_from(message.to_vec())?;
                        let market = unifier.get_market(&resp.symbol);
                        if market.is_none() {
                            return Ok(Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Unknown market {}", resp.symbol), None,
                            )))));
                        }
                        let market = market.unwrap();
                        let bids = resp.bids.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if bids.is_err() {
                            return Ok(Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid bid {:?}", resp.bids), Some(market),
                            )))));
                        }
                        let asks = resp.asks.iter().map(|b| b.try_into()).collect::<OrderBookResult<Vec<OrderBookUnit>>>();
                        if asks.is_err() {
                            return Ok(Some(StreamItem::OrderBook(Err(OrderBookError::InvalidOrderBook(
                                format!("Invalid ask {:?}", resp.asks), Some(market),
                            )))));
                        }
                        let book = OrderBook::new(
                            bids.unwrap(),
                            asks.unwrap(),
                            market,
                            Some(resp.event_time),
                            None,
                        );
                        Ok(Some(StreamItem::OrderBook(Ok(book))))
                    }
                    _ => {
                        let message = String::from_utf8_lossy(&message);
                        Ok(Some(StreamItem::Unknown(message.to_string())))
                    },
                }
            }))
            .channel_capacity(props.channel_capacity)
            .build()?;

        Ok(Self {
            exchange_base: ExchangeBase::new(&base_props)?,
            api_key: props.api_key.clone(),
            secret: props.secret.clone(),
            leverage_brackets: None,
        })
    }

    fn auth(&self, request: &String) -> Result<String> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials);
        }
        let mut signed_key = Hmac::<Sha256>::new_from_slice(self.secret.as_ref().unwrap().as_bytes())?;
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


    async fn load_leverage_brackets(&mut self) -> Result<()> {
        let mut query: Vec<(&str, &str)> = vec![];
        let ts = Utc::now().timestamp_millis().to_string();
        query.push(("timestamp", ts.as_str()));
        let signature = self.auth_map(Some(&query))?;
        query.push(("signature", signature.as_str()));
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];

        self.leverage_brackets = Some(HashMap::new());
        let result: Vec<FetchLeverageResponse> = self.exchange_base.http_client.get("/fapi/v1/leverageBracket", Some(headers), Some(&query)).await?;
        for resp in result {
            let market = self.exchange_base.unifier.get_market(&resp.symbol);
            match market {
                Some(market) => {
                    resp.brackets.iter().for_each(|b| {
                        self.leverage_brackets.as_mut().unwrap().entry(market.clone()).or_insert(vec![]).push(LeverageBracket {
                            notional_floor: b.notional_floor,
                            maintenance_margin_ratio: b.maint_margin_ratio,
                        });
                    });
                }
                None => continue,
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&mut self) -> LoadMarketResult<Vec<Market>> {
        if self.exchange_base.markets.is_empty() {
            let result = self.exchange_base.http_client.get::<(), FetchMarketsResponse>("/fapi/v1/exchangeInfo", None, None).await?;
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
        }
        if self.api_key.is_some() && self.secret.is_some() && self.leverage_brackets.is_none() {
            self.load_leverage_brackets().await?;
        }
        Ok(self.exchange_base.markets.clone())
    }

    async fn fetch_markets(&self) -> FetchMarketResult<Vec<Market>> {
        let result = self.exchange_base.http_client.get::<(), FetchMarketsResponse>("/fapi/v1/exchangeInfo", None, None).await?;
        let mut markets = vec![];
        for s in result.symbols {
            if s.symbol.is_none() {
                continue;
            }
            let market: Result<Market> = (&s).into();
            let market = market?;
            markets.push(market);
        }
        Ok(markets)
    }
    async fn fetch_tickers(&self, params: &FetchTickersParams) -> FetchTickersResult<Vec<Ticker>> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }

        let query = params.markets.as_ref().map(|markets| {
            let s = markets.iter()
                .map(|m| self.exchange_base.unifier.get_symbol_id(&m))
                .filter(|s| s.is_some())
                .map(|s| format!("\"{}\"", s.unwrap()))
                .collect::<Vec<String>>()
                .join(",");
            vec![("symbols", format!("[{}]", s))]
        });

        let result: Vec<FetchTickersResponse> = self.exchange_base.http_client.get("/fapi/v1/ticker/24hr", None, query.as_ref()).await?;
        let mut tickers = vec![];
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
                base_volume: item.volume.parse::<f64>()?,
                change: item.price_change.parse::<f64>()?,
                close: last,
                high: item.high_price.parse::<f64>()?,
                last,
                low: item.low_price.parse::<f64>()?,
                open: open,
                percentage: item.price_change_percent.parse::<f64>()?,
                previous_close: None,
                quote_volume: item.quote_volume.parse::<f64>()?,
                average: (open + last) / 2f64,
                market,
                timestamp,
                vwap: item.weighted_avg_price.parse::<f64>()?,
                ..Default::default()
            });
        }
        Ok(tickers)
    }

    async fn watch_order_book(&self, params: &WatchOrderBookParams) -> WatchOrderBookResult<Receiver> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }

        let markets = &params.markets;
        if markets.len() == 0 {
            return Err(Error::InvalidParameters("markets is empty".into()).into());
        }

        if self.exchange_base.ws_endpoint.is_none() {
            return Err(Error::InvalidParameters("ws endpoint is empty".into()).into());
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
                    .map(|s| format!("\"{}@depth5@100ms\"", s.to_lowercase()))
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

    async fn create_order(&self, params: &CreateOrderParams) -> CreateOrderResult<Order> {
        if self.api_key.is_none() || self.secret.is_none() {
            return Err(Error::InvalidCredentials.into());
        }

        let order_type = params.order_type.unwrap_or_default();
        if params.price.is_none() && order_type == OrderType::Limit {
            return Err(Error::InvalidPrice("price is required for limit orders".into()).into());
        }
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }
        let symbol_id = self.exchange_base.unifier.get_symbol_id(&params.market).ok_or(Error::SymbolNotFound(format!("{}", params.market)))?;
        let timestamp = Utc::now().timestamp_millis();
        let mut body = format!("symbol={}&side={}&type={}&quantity={}&timeInForce={}&recvWindow=5000&timestamp={}",
                               symbol_id,
                               util::get_exchange_order_side(&params.order_side),
                               util::get_exchange_order_type(&order_type)?,
                               params.amount,
                               util::get_exchange_time_in_force(&params.time_in_force.unwrap_or(TimeInForce::GTC)),
                               timestamp);
        if params.price.is_some() {
            body = format!("{}&price={}", body, params.price.unwrap());
        }
        let signature = self.auth(&body)?;
        let body = format!("{}&signature={}", body, signature);
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let response: CreateOrderResponse = self.exchange_base.http_client.post("/fapi/v1/order", Some(headers), EMPTY_QUERY, Some(&body)).await?;
        let mut order: Order = response.try_into()?;
        order.market = params.market.clone();
        order.order_type = order_type;
        Ok(order)
    }

    async fn fetch_balance(&self, params: &FetchBalanceParams) -> FetchBalanceResult<Balance> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }

        if params.margin_mode.is_some() && params.margin_mode.unwrap() != MarginMode::Cross {
            return Err(Error::InvalidParameters("margin mode is not supported".into()).into());
        }

        let mut query = vec![];
        let ts = Utc::now().timestamp_millis().to_string();
        query.push(("timestamp", ts.as_str()));
        let signature = self.auth_map(Some(&query))?;
        query.push(("signature", signature.as_str()));
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let resp: FetchBalanceResponse = self.exchange_base.http_client.get("/fapi/v2/account", Some(headers), Some(&query)).await?;
        let mut bal = Balance::default();
        bal.timestamp = None;

        for asset in resp.assets {
            let free = parse_float64(&asset.available_balance)?;
            let used = parse_float64(&asset.initial_margin)?;
            let total = parse_float64(&asset.margin_balance)?;
            let item = BalanceItem {
                currency: util::to_unified_asset(&asset.asset),
                market: None,
                total,
                free,
                used,
                debt: 0.0,
            };
            bal.items.push(item);
        }
        Ok(bal)
    }

    async fn fetch_positions(&self, _params: &FetchPositionsParams) -> FetchPositionsResult<Vec<Position>> {
        if self.exchange_base.markets.is_empty() {
            return Err(Error::MarketNotInitialized.into());
        }
        let mut query = vec![];
        let ts = Utc::now().timestamp_millis().to_string();
        query.push(("timestamp", ts.as_str()));
        let signature = self.auth_map(Some(&query))?;
        query.push(("signature", signature.as_str()));
        let headers = vec![("X-MBX-APIKEY", self.api_key.as_ref().unwrap().as_str())];
        let items: Vec<FetchPositionsResponse> = self.exchange_base.http_client.get("/fapi/v2/positionRisk", Some(headers), Some(&query)).await?;

        let mut ret = vec![];
        for item in items {
            let market = self.exchange_base.unifier.get_market(&item.symbol);
            if market.is_none() {
                continue;
            }
            let market = market.unwrap();

            let notional: f64 = item.notional.parse().map_err(|_| Error::ParseError(item.notional))?;
            let abs_notional = notional.abs();

            let maintenance_margin_percent = self.leverage_brackets
                .as_ref()
                .and_then(|leverage_brackets| leverage_brackets.get(&market))
                .and_then(|brackets| brackets.iter().find(|b| abs_notional >= b.notional_floor))
                .map(|b| b.maintenance_margin_ratio);

            let maintenance_margin_percent = maintenance_margin_percent.ok_or_else(|| Error::InvalidResponse("maintenance margin ratio is not found".into()))?;
            let maintenance_margin = abs_notional * maintenance_margin_percent;

            let margin_mode = match item.margin_type.as_str() {
                "cross" => MarginMode::Cross,
                _ => MarginMode::Isolated,
            };
            let is_hedged = match item.position_side.as_str() {
                "BOTH" => false,
                _ => true,
            };

            let side = match notional {
                n if n > 0.0 => PositionSide::Long,
                n if n < 0.0 => PositionSide::Short,
                _ => continue,
            };
            let contracts: f64 = item.position_amt.parse::<f64>().map_err(|_| Error::ParseError(item.position_amt))?.abs();
            let liquidation_price = item.liquidation_price.parse().map_err(|_| Error::ParseError(item.liquidation_price))?;
            let entry_price: f64 = item.entry_price.parse().map_err(|_| Error::ParseError(item.entry_price))?;
            let leverage = item.leverage.parse().map_err(|_| Error::ParseError(item.leverage))?;

            let collateral: f64 = match margin_mode {
                MarginMode::Cross => {
                    // walletBalance = (liquidationPrice * (±1 + mmp) ± entryPrice) * contracts
                    let (mmp, entry_price) = match side {
                        PositionSide::Long => {
                            (-1f64 + maintenance_margin_percent, entry_price)
                        }
                        PositionSide::Short => {
                            (1f64 + maintenance_margin_percent, -entry_price)
                        }
                    };
                    (liquidation_price * mmp + entry_price) * contracts.abs()
                }
                MarginMode::Isolated => item.isolated_margin.parse().map_err(|_| Error::ParseError(item.isolated_margin))?,
            };

            let initial_margin_percent = 1f64 / leverage;
            let initial_margin = abs_notional * initial_margin_percent;

            let margin_ratio = maintenance_margin / collateral + 5e-5;
            let unrealized_pnl = item.un_realized_profit.parse::<f64>().map_err(|_| Error::ParseError(item.un_realized_profit))?;
            let percentage = unrealized_pnl / initial_margin * 100f64;

            ret.push(Position {
                market: market.clone(),
                side,
                contracts,
                contract_size: market.contract_size,
                unrealized_pnl,
                leverage,
                liquidation_price,
                collateral,
                notional: abs_notional,
                mark_price: item.mark_price.parse().map_err(|_| Error::ParseError(item.mark_price))?,
                entry_price,
                timestamp: item.update_time,

                initial_margin,
                initial_margin_percent,
                maintenance_margin_percent,
                maintenance_margin,

                margin_ratio,
                margin_mode,
                is_hedged,
                percentage,
                ..Default::default()
            });
        }
        Ok(ret)
    }
}

struct LeverageBracket {
    notional_floor: f64,
    maintenance_margin_ratio: f64,
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

impl TryFrom<Vec<u8>> for WatchOrderBookResponse {
    type Error = Error;

    fn try_from(message: Vec<u8>) -> Result<Self> {
        serde_json::from_slice(&message).map_err(|e| {
            let message = String::from_utf8_lossy(&message);
            Error::DeserializeJsonBody(format!("Failed to deserialize json body. message={:?}, error={:?}", message, e))
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
}

impl TryFrom<Vec<u8>> for WatchCommonResponse {
    type Error = Error;

    fn try_from(message: Vec<u8>) -> Result<Self> {
        serde_json::from_slice(&message).map_err(|e| {
            let message = String::from_utf8_lossy(&message);
            Error::DeserializeJsonBody(format!("Failed to deserialize json body. message={:?}, error={:?}", message, e))
        })
    }
}


#[derive(Serialize, Deserialize)]
struct ErrorResponse {
    code: i64,
    msg: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateOrderResponse {
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
        let order_status = util::get_unified_order_status(&resp.status)?;
        let amount = resp.orig_qty.parse()?;
        let remaining = match order_status {
            OrderStatus::Open => Some(amount),
            _ => None,
        };
        Ok(Order {
            id: Some(resp.order_id.to_string()),
            client_order_id: Some(resp.client_order_id),
            timestamp: resp.update_time,
            status: order_status,
            time_in_force: Some(util::get_unified_time_in_force(&resp.time_in_force)?),
            side: Some(util::get_unified_order_side(&resp.side)?),
            price: Some(resp.price.parse()?),
            average: Some(resp.avg_price.parse()?),
            amount: resp.orig_qty.parse()?,
            remaining,
            ..Default::default()
        })
    }
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchLeverageBracketResponse {
    pub bracket: f64,
    #[serde(rename = "initialLeverage")]
    pub initial_leverage: f64,
    #[serde(rename = "notionalCap")]
    pub notional_cap: f64,
    #[serde(rename = "notionalFloor")]
    pub notional_floor: f64,
    #[serde(rename = "maintMarginRatio")]
    pub maint_margin_ratio: f64,
    pub cum: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct FetchLeverageResponse {
    pub symbol: String,
    #[serde(rename = "notionalCoef")]
    pub notional_coef: Option<f64>,
    pub brackets: Vec<FetchLeverageBracketResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMarketsResponse {
    pub exchange_filters: Option<Vec<String>>,
    pub rate_limits: Vec<FetchMarketsRateLimitResponse>,
    pub server_time: i64,
    pub assets: Option<Vec<FetchMarketsAssetResponse>>,
    pub symbols: Vec<FetchMarketsSymbolResponse>,
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


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMarketsRateLimitResponse {
    pub interval: String,
    pub interval_num: i64,
    pub limit: i64,
    pub rate_limit_type: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMarketsAssetResponse {
    pub asset: String,
    pub margin_available: bool,
    pub auto_asset_exchange: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMarketsSymbolResponse {
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
    pub filters: Option<Vec<FetchMarketsFilterResponse>>,
    pub order_type: Option<Vec<String>>,
    pub time_in_force: Option<Vec<String>>,
    pub liquidation_fee: Option<String>,
    pub market_take_bound: Option<String>,
}

impl Into<Result<Market>> for &FetchMarketsSymbolResponse {
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
            Some(ref s) if s == "CURRENT_QUARTER" => MarketType::Future,
            Some(ref s) if s == "NEXT_QUARTER" => MarketType::Future,
            _ => MarketType::Unknown,
        };

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
                }
                "MIN_NOTIONAL" => {
                    let min = filter.notional.as_ref().ok_or_else(|| Error::MissingField("notional".into()))?.parse::<f64>()?;
                    limit.cost = Some(Range { min, max: f64::MAX });
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
            contract_size: Some(1.0),
            contract_type: Some(ContractType::Linear),
            expiry: self.delivery_date,
            precision: Some(precision),
            limit: Some(limit),
            ..Default::default()
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchMarketsFilterResponse {
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


#[derive(Serialize, Deserialize, Debug)]
struct FetchBalancePositionResponse {
    pub symbol: String,
    #[serde(rename = "initialMargin")]
    pub initial_margin: String,
    #[serde(rename = "maintMargin")]
    pub maint_margin: String,
    #[serde(rename = "unrealizedProfit")]
    pub unrealized_profit: String,
    #[serde(rename = "positionInitialMargin")]
    pub position_initial_margin: String,
    #[serde(rename = "openOrderInitialMargin")]
    pub open_order_initial_margin: String,
    pub leverage: String,
    pub isolated: bool,
    #[serde(rename = "entryPrice")]
    pub entry_price: String,
    #[serde(rename = "breakEvenPrice")]
    pub break_even_price: Option<String>,
    #[serde(rename = "maxNotional")]
    pub max_notional: String,
    #[serde(rename = "bidNotional")]
    pub bid_notional: String,
    #[serde(rename = "askNotional")]
    pub ask_notional: String,
    #[serde(rename = "positionSide")]
    pub position_side: String,
    #[serde(rename = "positionAmt")]
    pub position_amt: String,
    #[serde(rename = "updateTime")]
    pub update_time: i64,
}


#[derive(Serialize, Deserialize, Debug)]
struct FetchBalanceAssetResponse {
    pub asset: String,
    #[serde(rename = "walletBalance")]
    pub wallet_balance: String,
    #[serde(rename = "unrealizedProfit")]
    pub unrealized_profit: String,
    #[serde(rename = "marginBalance")]
    pub margin_balance: String,
    #[serde(rename = "maintMargin")]
    pub maint_margin: String,
    #[serde(rename = "initialMargin")]
    pub initial_margin: String,
    #[serde(rename = "positionInitialMargin")]
    pub position_initial_margin: String,
    #[serde(rename = "openOrderInitialMargin")]
    pub open_order_initial_margin: String,
    #[serde(rename = "crossWalletBalance")]
    pub cross_wallet_balance: String,
    #[serde(rename = "crossUnPnl")]
    pub cross_un_pnl: String,
    #[serde(rename = "availableBalance")]
    pub available_balance: String,
    #[serde(rename = "maxWithdrawAmount")]
    pub max_withdraw_amount: String,
    #[serde(rename = "marginAvailable")]
    pub margin_available: bool,
    #[serde(rename = "updateTime")]
    pub update_time: i64,
}

#[derive(Serialize, Deserialize, Debug)]
struct FetchBalanceResponse {
    #[serde(rename = "feeTier")]
    pub fee_tier: i64,
    #[serde(rename = "canTrade")]
    pub can_trade: bool,
    #[serde(rename = "canDeposit")]
    pub can_deposit: bool,
    #[serde(rename = "canWithdraw")]
    pub can_withdraw: bool,
    #[serde(rename = "updateTime")]
    pub update_time: i64,
    #[serde(rename = "multiAssetsMargin")]
    pub multi_assets_margin: bool,
    #[serde(rename = "tradeGroupId")]
    pub trade_group_id: i64,
    #[serde(rename = "totalInitialMargin")]
    pub total_initial_margin: String,
    #[serde(rename = "totalMaintMargin")]
    pub total_maint_margin: String,
    #[serde(rename = "totalWalletBalance")]
    pub total_wallet_balance: String,
    #[serde(rename = "totalUnrealizedProfit")]
    pub total_unrealized_profit: String,
    #[serde(rename = "totalMarginBalance")]
    pub total_margin_balance: String,
    #[serde(rename = "totalPositionInitialMargin")]
    pub total_position_initial_margin: String,
    #[serde(rename = "totalOpenOrderInitialMargin")]
    pub total_open_order_initial_margin: String,
    #[serde(rename = "totalCrossWalletBalance")]
    pub total_cross_wallet_balance: String,
    #[serde(rename = "totalCrossUnPnl")]
    pub total_cross_un_pnl: String,
    #[serde(rename = "availableBalance")]
    pub available_balance: String,
    #[serde(rename = "maxWithdrawAmount")]
    pub max_withdraw_amount: String,
    pub assets: Vec<FetchBalanceAssetResponse>,
    pub positions: Vec<FetchBalancePositionResponse>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchPositionsResponse {
    pub symbol: String,
    pub position_amt: String,
    pub entry_price: String,
    pub break_even_price: String,
    pub mark_price: String,
    pub un_realized_profit: String,
    pub liquidation_price: String,
    pub leverage: String,
    pub max_notional_value: String,
    pub margin_type: String,
    pub isolated_margin: String,
    pub is_auto_add_margin: String,
    pub position_side: String,
    pub notional: String,
    pub isolated_wallet: String,
    pub update_time: i64,
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
    #[serde(rename = "lastPrice")]
    pub last_price: String,
    #[serde(rename = "lastQty")]
    pub last_qty: String,
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


#[cfg(test)]
mod test {
    use crate::{BinanceUsdm, Exchange, FetchBalanceParamsBuilder, FetchTickersParamsBuilder, PropertiesBuilder};
    use crate::exchange::params::FetchPositionsParamsBuilder;
    use crate::model::{MarginMode, MarketType};

    #[tokio::test]
    async fn test_auth() {
        let api_key = "dbefbc809e3e83c283a984c3a1459732ea7db1360ca80c5c2c8867408d28cc83";
        let secret = "2b5eb11e18796d12d88f13dc27dbbd02c2cc51ff7059765ed9821957d82bb4d9";

        let props = PropertiesBuilder::default().api_key(Some(api_key.to_string())).secret(Some(secret.to_string())).build().expect("failed to create properties");
        let exchange = BinanceUsdm::new(props).expect("failed to create exchange");
        let mut params = vec![];
        params.push(("symbol", "BTCUSDT"));
        params.push(("side", "BUY"));
        params.push(("type", "LIMIT"));
        params.push(("quantity", "1"));
        params.push(("price", "9000"));
        params.push(("timeInForce", "GTC"));
        params.push(("recvWindow", "5000"));
        params.push(("timestamp", "1591702613943"));
        let result = exchange.auth_map(Some(&params));
        assert_eq!(result.unwrap(), "3c661234138461fcc7a7d8746c6558c9842d4e10870d2ecbedf7777cad694af9");
    }

    #[tokio::test]
    async fn test_load_leverage_brackets() {
        let api_key = std::env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY is not set");
        let secret = std::env::var("BINANCE_SECRET").expect("BINANCE_SECRET is not set");

        let props = PropertiesBuilder::default().api_key(Some(api_key)).secret(Some(secret)).build().expect("failed to create properties");
        let mut exchange = BinanceUsdm::new(props).expect("failed to create exchange");
        let result = exchange.load_leverage_brackets().await;
        println!("{:?}", result);
        assert!(!result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_positions() {
        let api_key = std::env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY is not set");
        let secret = std::env::var("BINANCE_SECRET").expect("BINANCE_SECRET is not set");

        let props = PropertiesBuilder::default().api_key(Some(api_key)).secret(Some(secret)).build().expect("failed to create properties");
        let mut exchange = BinanceUsdm::new(props).expect("failed to create exchange");
        exchange.load_markets().await.expect("failed to load markets");
        let params = FetchPositionsParamsBuilder::default().build().expect("failed to create params");
        let result = exchange.fetch_positions(&params).await;
        for p in result.unwrap() {
            if p.notional != 0.0 {
                println!("{:?}", p);
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_balance() {
        let api_key = std::env::var("BINANCE_API_KEY").expect("BINANCE_API_KEY is not set");
        let secret = std::env::var("BINANCE_SECRET").expect("BINANCE_SECRET is not set");

        let props = PropertiesBuilder::default().api_key(Some(api_key)).secret(Some(secret)).build().expect("failed to create properties");
        let mut exchange = BinanceUsdm::new(props).expect("failed to create exchange");
        exchange.load_markets().await.expect("failed to load markets");
        let params = FetchBalanceParamsBuilder::default().margin_mode(Some(MarginMode::Cross)).build().expect("failed to create params");
        let result = exchange.fetch_balance(&params).await;
        for item in result.unwrap().items {
            if item.currency == "USDT" {
                println!("{:?}", item);
            }
        }
    }

    #[tokio::test]
    async fn test_fetch_tickers() {
        let mut exchange = BinanceUsdm::new(PropertiesBuilder::default().build().unwrap()).unwrap();
        let markets = exchange.load_markets().await.unwrap();
        let target_market = markets.into_iter().find(|m| m.base == "BTC" && m.quote == "USDT" && m.market_type == MarketType::Swap).unwrap();
        let params = FetchTickersParamsBuilder::default().markets(Some(vec![target_market])).build().unwrap();
        let tickers = exchange.fetch_tickers(&params).await;
        tickers.unwrap().iter().for_each(|ticker| {
            println!("{:?}", ticker);
        });
    }
}
