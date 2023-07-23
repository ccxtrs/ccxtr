use std::hash::{Hash, Hasher};
use std::ops;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::Error;

pub type Decimal = rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Hash, Debug, Clone, Serialize, Deserialize)]
pub enum ContractType {
    Linear,
    Inverse,
}

#[derive(Hash, PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum MarketType {
    Spot,
    Margin,
    Swap,
    Futures,
    Option,
    Unknown,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeeSide {
    Get,
    Give,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fee {
    TakerBasisPoints(Decimal),
    MakerBasisPoints(Decimal),
    TakerFixedAmount(Decimal),
    MakerFixedAmount(Decimal),
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    /// string literal for referencing within an exchange
    pub id: String,

    /// uppercase string literal of a pair of currencies
    pub symbol: String,

    /// uppercase string, unified base currency code, 3 or more letters
    pub base: String,

    /// uppercase string, unified quote currency code, 3 or more letters
    pub quote: String,

    /// any string, exchange-specific base currency id
    pub base_id: String,

    /// any string, exchange-specific quote currency id
    pub quote_id: String,

    /// boolean, market status
    pub active: bool,

    /// spot for spot, margin for margin, future for expiry futures, swap for perpetual swaps,
    /// option for options
    pub market_type: MarketType,

    /// the unified currency code that the contract will settle in, only set if `market_type` is a
    /// future, a swap or an option
    pub settle: Option<String>,

    /// the currencyId of that the contract will settle in, only set if `market_type` is a future, a
    /// swap or an option.
    pub settle_id: Option<String>,

    /// the size of one contract, only used if `market_type` is a future, a swap or an option.
    pub contract_size: Option<Decimal>,

    /// linear or inverse, only used if `market_type` is a future, a swap or an option.
    pub contract_type: Option<ContractType>,

    /// the unix expiry timestamp in milliseconds, None for everything except future market type.
    pub expiry: Option<i64>,

    /// The datetime contract will in iso8601 format
    pub expiry_datetime: String,

    /// price at which a put or call option can be exercised
    pub strike: Option<Decimal>,

    /// call or put, call option represents an option with the right to buy and put an option
    /// with the right to sell
    pub option_type: Option<OptionType>,

    /// taker and maker, and whether a basis point or a fixed amount
    pub fee: Option<Fee>,

    /// uppercase string. unified currency code, 3 or more letters
    pub fee_currency: Option<String>,

    /// any string, exchange-specific currency id
    pub fee_currency_id: Option<String>,

    /// get or give
    pub fee_side: Option<FeeSide>,

    /// precision for price, amount and cost
    pub precision: Option<Precision>,

    /// market limits for amount, price, cost and leverage
    pub limit: Option<MarketLimit>,
}

impl Hash for Market {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
        self.quote.hash(state);
        self.market_type.hash(state);
        self.expiry.hash(state);
    }
}

impl PartialEq<Self> for Market {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
            && self.quote == other.quote
            && self.market_type == other.market_type
            && self.expiry == other.expiry
    }
}

impl Eq for Market {}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precision {
    /// number of decimal digits after the decimal point
    pub price: Option<isize>,

    /// number of decimal digits after the decimal point
    pub amount: Option<isize>,

    /// number of decimal digits after the decimal point
    pub cost: Option<isize>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLimit {
    pub amount: Option<Range>,
    pub price: Option<Range>,

    /// cost = price * amount
    pub cost: Option<Range>,
    pub leverage: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Currency {
    /// The string or numeric ID of the currency within the exchange. Currency ids are used inside
    /// exchanges internally to identify coins during the request/response process.
    pub id: String,

    /// An uppercase string code representation of a particular currency. Currency codes are used
    /// to reference currencies within the ccxtr library.
    pub code: String,

    /// A human-readable name of the currency (can be a mix of uppercase & lowercase characters).
    pub name: String,

    /// A boolean indicating whether trading or funding (depositing or withdrawing) for this
    /// currency is currently possible, more about it here: active status.
    pub active: bool,

    /// The withdrawal fee value as specified by the exchange. In most cases it means a flat fixed
    /// amount paid in the same currency. If the exchnange does not specify it via public endpoints,
    /// the fee can be None.
    pub fee: Option<Decimal>,

    /// Precision accepted in values by exchanges upon referencing this currency. The value of this
    /// property depends on exchange.precisionMode.
    pub precision: isize,

    /// deposits are available.
    pub deposit: bool,

    /// withdraws are available.
    pub withdraw: bool,

    /// The minimums and maximums for amounts (volumes), withdrawals and deposits.
    pub limits: CurrencyLimit,

    /// network structures indexed by unified network identifiers (ERC20, TRC20, BSC, etc)
    pub network: Network,
}

pub type Range = ops::Range<Decimal>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyLimit {
    pub amount: Option<Range>,
    pub withdraw: Option<Range>,
    pub deposit: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// The string or numeric ID of the network within the exchange. Network ids are used inside
    /// exchanges internally to identify networks during the request/response process.
    pub id: String,

    /// An uppercase string representation of a particular network. Networks are used to reference
    /// networks within the ccxtr library.
    pub network: String,

    /// A human-readable name of the network (can be a mix of uppercase & lowercase characters).
    pub name: String,

    /// A boolean indicating whether trading or funding (depositing or withdrawing) for this
    /// currency is currently possible, more about it here: active status.
    pub active: bool,

    /// The withdrawal fee value as specified by the exchange. In most cases it means a flat fixed
    /// amount paid in the same currency. If the exchnange does not specify it via public endpoints,
    /// the fee can be None.
    pub fee: Option<Decimal>,

    /// Precision accepted in values by exchanges upon referencing this currency. The value of this
    /// property depends on exchange.precisionMode.
    pub precision: isize,

    /// deposits are available
    pub deposit: bool,

    /// withdraws are available
    pub withdraw: bool,

    /// The minimums and maximums for amounts (volumes), withdrawals and deposits.
    pub limits: CurrencyLimit,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub bids: Vec<OrderBookUnit>,
    pub asks: Vec<OrderBookUnit>,
    pub symbol: String,
    pub timestamp: i64,
    pub datetime: String,
    pub nonce: Option<i64>
}

impl OrderBook {
    pub fn new() -> Self {
        Self{
            bids: Vec::new(),
            asks: Vec::new(),
            symbol: String::new(),
            timestamp: 0,
            datetime: String::new(),
            nonce: None
        }
    }
}

impl From<String> for OrderBook {
    fn from(value: String) -> Self {
        serde_json::from_str(&value).unwrap()
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookUnit {
    pub price: Decimal,
    pub amount: Decimal,
}