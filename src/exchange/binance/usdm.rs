use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use chrono::LocalResult::Single;
use futures::SinkExt;
use futures::channel::mpsc::Receiver;
use serde::{Deserialize, Serialize};

use crate::{client::NONE, Error, exchange::{Exchange, Properties}, model::{Market, MarketType}, PropertiesBuilder, Result};
use crate::exchange::{ExchangeBase, StreamItem};
use crate::exchange::binance::util;
use crate::model::{ContractType, Decimal, OrderBook};
use crate::model::{MarketLimit, Precision, Range};
use crate::util::into_precision;

pub struct BinanceUsdm {
    exchange_base: ExchangeBase,
}


impl BinanceUsdm {
    pub fn new(props: Properties) -> Result<Self> {
        let common_props = PropertiesBuilder::new()
            .host(props.host.unwrap_or_else(|| "https://fapi.binance.com".into()))
            .port(props.port.unwrap_or(443))
            .ws_endpoint("wss://fstream.binance.com/ws")
            .api_key(props.api_key.unwrap_or_default())
            .secret_key(props.secret_key.unwrap_or_default())
            .stream_parser(|message| {
                StreamItem::OrderBook(OrderBook::new())
            });


        Ok(Self {
            exchange_base: ExchangeBase::new(common_props.build())?,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        self.exchange_base.connect().await
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&mut self) -> Result<&Vec<Market>> {
        if self.exchange_base.markets.is_empty() {
            self.fetch_markets().await?;
        }
        Ok(&self.exchange_base.markets)
    }

    async fn fetch_markets(&mut self) -> Result<&Vec<Market>> {
        let result: FetchMarketsResponse = self.exchange_base.http_client.get("/fapi/v1/exchangeInfo", NONE).await?;
        let result: Result<Vec<Market>> = result.try_into();
        match result {
            Ok(markets) => {
                for m in &markets {
                    self.exchange_base.unifier.insert_market_symbol_id(&m, &m.id).await;
                }
                self.exchange_base.markets = markets;
                Ok(&self.exchange_base.markets)
            }
            Err(e) => Err(e),
        }
    }

    async fn watch_order_book(&mut self, markets: Vec<Market>) -> Result<Receiver<OrderBook>> {
        let mut sender = self.exchange_base.ws_client.sender()
            .ok_or(Error::WebsocketError("no sender".into()))?;

        let mut symbol_ids: Vec<String> = Vec::new();
        for m in &markets {
            match self.exchange_base.unifier.get_symbol_id(&m).await {
                Some(symbol_id) => symbol_ids.push(symbol_id),
                None => return Err(Error::SymbolNotFound(m.symbol.clone())),
            }
        }
        let params = symbol_ids.iter()
            .map(|s| format!("\"{}@depth5@100ms\"", s.to_lowercase()))
            .collect::<Vec<String>>()
            .join(",");

        let stream_name = format!("{{\"method\": \"SUBSCRIBE\", \"params\": [{params}], \"id\": 1}}");
        sender.send(stream_name).await?;

        self.exchange_base.order_book_stream.take()
            .ok_or(Error::WebsocketError("no receiver".into()))
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
            .map(|s| s.into())
            .collect()
    }
}


impl Into<Result<Market>> for Symbol {
    fn into(self) -> Result<Market> {
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

