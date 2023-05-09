use crate::{
    exchange::{Exchange, Properties},
    Result,
    client::NONE,
    model::{Market, MarketType},
    Error,
};

use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use chrono::LocalResult::Single;
use crate::model::{ContractType, Decimal, OrderBook};
use crate::exchange::binance::util;
use crate::model::{CurrencyLimit, MarketLimit, Precision, Range};
use crate::util::into_precision;

pub struct BinanceUsdm {
    http_client: crate::client::HttpClient,
    ws_client: crate::client::WsClient,
    #[allow(dead_code)]
    api_key: Option<String>,
    #[allow(dead_code)]
    secret_key: Option<String>,
}


impl BinanceUsdm {
    pub async fn new(props: Properties) -> Result<Self> {
        let host = props.host.unwrap_or_else(|| "https://fapi.binance.com".into());
        let port = props.port.unwrap_or(443);

        let http_client = crate::client::Builder::new().host(host).port(port).build()?;
        let ws_client = crate::client::WsClient::new("wss://fstream.binance.com/ws").await?;

        Ok(Self {
            api_key: props.api_key,
            secret_key: props.secret_key,
            http_client,
            ws_client: ws_client
        })
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&self) -> Result<Vec<Market>> {
        self.fetch_markets().await
    }

    async fn fetch_markets(&self) -> Result<Vec<Market>> {
        let result: Result<LoadMarketsResponse> =
            self.http_client.get("/fapi/v1/exchangeInfo", NONE).await;
        result?.try_into()
    }

    async fn watch_order_book(&self) -> tokio_stream<OrderBook> {

    }
}


#[derive(Serialize)]
struct LoadMarketsRequest {}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in super) struct LoadMarketsResponse {
    pub exchange_filters: Option<Vec<String>>,
    pub rate_limits: Vec<RateLimit>,
    pub server_time: i64,
    pub assets: Option<Vec<Asset>>,
    pub symbols: Vec<Symbol>,
    pub timezone: String,
}

impl TryInto<Vec<Market>> for LoadMarketsResponse {
    type Error = Error;

    fn try_into(self) -> std::result::Result<Vec<Market>, Self::Error> {
        self.symbols.into_iter()
            .map(|s| s.into())
            .collect()
    }
}


impl Into<std::result::Result<Market, Error>> for Symbol {
    fn into(self) -> std::result::Result<Market, Error> {
        let base_id = self.base_asset.ok_or_else(|| Error::MissingField("base_asset".into()))?;
        let quote_id = self.quote_asset.ok_or_else(|| Error::MissingField("quote_asset".into()))?;
        let settle_id = self.margin_asset;


        let base = util::to_unified_symbol(&base_id);
        let quote = util::to_unified_symbol(&quote_id);
        let settle = settle_id.as_ref()
            .and_then(|s| Some(util::to_unified_symbol(s)));

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

        let symbol = match market_type {
            MarketType::Swap => format!("{base}/{quote}"),
            MarketType::Futures => format!("{base}/{quote}:{}", delivery_date
                .and_then(|dt| Some(dt.format("%Y%m%d").to_string()))
                .unwrap_or_else(|| "".into())),
            _ => format!("{base}/{quote}"),
        };

        let active = util::is_active(self.status);

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
                    let start = filter.min_price.as_ref().ok_or_else(|| Error::MissingField("min_price".into()))?.parse::<Decimal>()?;
                    let end = filter.max_price.as_ref().ok_or_else(|| Error::MissingField("max_price".into()))?.parse::<Decimal>()?;
                    limit.price = Some(Range { start, end });
                    let tick_size = filter.tick_size.as_ref().ok_or_else(|| Error::MissingField("tick_size".into()))?;
                    precision.price = Some(into_precision(tick_size.clone())?);
                }
                "LOT_SIZE" => {
                    let start = filter.min_qty.as_ref().ok_or_else(|| Error::MissingField("min_qty".into()))?.parse::<Decimal>()?;
                    let end = filter.max_qty.as_ref().ok_or_else(|| Error::MissingField("max_qty".into()))?.parse::<Decimal>()?;
                    limit.amount = Some(Range { start, end });
                }
                "MIN_NOTIONAL" => {
                    let start = filter.notional.as_ref().ok_or_else(|| Error::MissingField("notional".into()))?.parse::<Decimal>()?;
                    limit.cost = Some(Range { start, end: Decimal::MAX });
                }
                // "MARKET_LOT_SIZE" => {},
                // "MAX_NUM_ORDERS" => {},
                // "MAX_NUM_ALGO_ORDERS" => {},
                // "PERCENT_PRICE" => {},
                _ => {}
            }
        }
        Ok(Market {
            id: self.symbol.ok_or_else(|| Error::MissingField("symbol".into()))?,
            symbol,
            base,
            quote,
            base_id,
            quote_id,
            active,
            market_type,
            settle,
            settle_id,
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

