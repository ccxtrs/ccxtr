use crate::{
    exchange::{Exchange, Properties},
    Result,
    client::NONE,
    model::{Market, MarketType},
    Error,
};

use serde::{Serialize, Deserialize};
use async_trait::async_trait;

pub struct BinanceUsdm {
    client: crate::client::HttpClient,
    #[allow(dead_code)]
    api_key: Option<String>,
    #[allow(dead_code)]
    secret_key: Option<String>,
}


impl BinanceUsdm {
    pub fn new(props: Properties) -> Self {
        let host = props.host.unwrap_or_else(|| "https://api.binance.com".to_string());
        let port = props.port.unwrap_or(443);

        let client = crate::client::Builder::new(host, port)
            .build();

        Self {
            api_key: props.api_key,
            secret_key: props.secret_key,
            client,
        }
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&self) -> Result<Vec<Market>> {
        let result: Result<LoadMarketsResponse> =
            self.client.get("/api/v1/exchangeInfo", NONE).await;
        result?.try_into()
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
    type Error = crate::Error;

    fn try_into(self) -> std::result::Result<Vec<Market>, Self::Error> {
        self.symbols.into_iter()
            .map(|s| s.into())
            .collect()
    }
}


impl Into<std::result::Result<Market, Error>> for Symbol {
    fn into(self) -> std::result::Result<Market, Error> {
        let base = self.base_asset.ok_or_else(|| crate::Error::MissingField("base_asset".to_string()))?;
        let quote = self.quote_asset.ok_or_else(|| crate::Error::MissingField("quote_asset".to_string()))?;
        Ok(Market {
            id: "".to_string(),
            symbol: "".to_string(),
            base,
            quote,
            base_id: "".to_string(),
            quote_id: "".to_string(),
            active: false,
            market_type: MarketType::Spot,
            settle: None,
            settle_id: None,
            contract_size: None,
            contract_type: None,
            expiry: None,
            expiry_datetime: "".to_string(),
            strike: None,
            option_type: None,
            fee: None,
            fee_currency: None,
            fee_currency_id: None,
            fee_side: None,
            precision: None,
            limit: None,
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
    pub auto_asset_exchange: Option<i64>,
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
    pub price_precision: Option<i64>,
    pub quantity_precision: Option<i64>,
    pub base_asset_precision: Option<i64>,
    pub quote_precision: Option<i64>,
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
    pub multiplier_decimal: Option<i64>,
}

