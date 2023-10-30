use std::fmt::{Display, Error, Formatter};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::{OrderBookError, OrderBookResult};
use crate::util::timestamp_format;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContractType {
    Linear,
    Inverse,
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MarketType {
    Spot,
    Margin,
    Swap,
    Future,
    Option,
    Unknown,
}

impl Default for MarketType {
    fn default() -> Self {
        MarketType::Unknown
    }
}

impl Display for MarketType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketType::Spot => write!(f, "SPOT"),
            MarketType::Margin => write!(f, "MARGIN"),
            MarketType::Swap => write!(f, "SWAP"),
            MarketType::Future => write!(f, "FUTURE"),
            MarketType::Option => write!(f, "OPTION"),
            MarketType::Unknown => write!(f, "UNKNOWN"),
        }
    }
}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FeeSide {
    Get,
    Give,
    Base,
    Quote,
    Other,
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MarketFee {
    TakerBasisPoints(f64),
    MakerBasisPoints(f64),
    TakerFixedAmount(f64),
    MakerFixedAmount(f64),
}


#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Market {
    /// uppercase string, unified base currency code, 3 or more letters
    pub base: String,

    /// uppercase string, unified quote currency code, 3 or more letters
    pub quote: String,

    /// boolean, market status
    pub active: bool,

    /// spot for spot, margin for margin, future for expiry futures, swap for perpetual swaps,
    /// option for options
    pub market_type: MarketType,

    /// the unified currency code that the contract will settle in, only set if `market_type` is a
    /// future, a swap or an option
    pub settle: Option<String>,

    /// the size of one contract, only used if `market_type` is a future, a swap or an option.
    pub contract_size: Option<f64>,

    /// linear or inverse, only used if `market_type` is a future, a swap or an option.
    pub contract_type: Option<ContractType>,

    /// the unix expiry timestamp in milliseconds, None for everything except future market type.
    pub expiry: Option<i64>,

    /// price at which a put or call option can be exercised
    pub strike: Option<f64>,

    /// call or put, call option represents an option with the right to buy and put an option
    /// with the right to sell
    pub option_type: Option<OptionType>,

    /// taker and maker, and whether a basis point or a fixed amount
    pub fee: Option<MarketFee>,

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

impl Display for Market {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)?;
        if self.market_type == MarketType::Future || self.market_type == MarketType::Swap || self.market_type == MarketType::Option {
            let settle = self.settle.as_ref().unwrap_or(&self.quote);
            write!(f, ":{}", settle)?;
        }
        if self.market_type == MarketType::Future || self.market_type == MarketType::Option {
            if self.expiry.is_none() {
                write!(f, "-UNKNOWN")?;
            } else {
                let delivery = timestamp_format(self.expiry.unwrap(), "%y%m%d").map_err(|_| Error)?;
                write!(f, "-{}", delivery)?;
            }
        }
        if self.market_type == MarketType::Option {
            if self.strike.is_none() {
                write!(f, "-UNKNOWN")?;
            } else {
                write!(f, "-{}", self.strike.unwrap())?;
            }
        }

        Ok(())
    }
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


#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Precision {
    /// number of decimal digits after the decimal point
    pub price: Option<isize>,

    /// number of decimal digits after the decimal point
    pub amount: Option<isize>,

    /// number of decimal digits after the decimal point
    pub cost: Option<isize>,
}


#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarketLimit {
    pub amount: Option<Range>,
    pub price: Option<Range>,

    /// cost = price * amount
    pub cost: Option<Range>,
    pub leverage: Option<Range>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Currency {
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
    pub fee: Option<f64>,

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


#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CurrencyLimit {
    pub amount: Option<Range>,
    pub withdraw: Option<Range>,
    pub deposit: Option<Range>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Network {
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
    pub fee: Option<f64>,

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


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Position {
    /// string, position id to reference the position, similar to an order id
    pub id: Option<String>,

    /// market
    pub market: Market,

    /// integer unix time since 1st Jan 1970 in milliseconds
    pub timestamp: i64,

    /// whether or not the position is hedged, i.e. if trading in the opposite direction will close this position or make a new one
    pub is_hedged: bool,

    /// long or short
    pub side: PositionSide,

    /// number of contracts bought, aka the amount or size of the position
    pub contracts: f64,

    /// the size of one contract in quote units
    pub contract_size: Option<f64>,

    /// the average entry price of the position
    pub entry_price: f64,

    /// a price that is used for funding calculations
    pub mark_price: f64,

    /// the value of the position in the settlement currency
    pub notional: f64,

    /// the leverage of the position, related to how many contracts you can buy with a given amount of collateral
    pub leverage: f64,

    /// the maximum amount of collateral that can be lost, affected by pnl
    pub collateral: f64,

    /// the amount of collateral that is locked up in this position
    pub initial_margin: f64,

    /// the minimum amount of collateral needed to avoid being liquidated
    pub maintenance_margin: f64,

    /// the initialMargin as a percentage of the notional
    pub initial_margin_percent: f64,

    /// the maintenanceMargin as a percentage of the notional
    pub maintenance_margin_percent: f64,

    /// the difference between the market price and the entry price times the number of contracts, can be negative
    pub unrealized_pnl: f64,

    /// the price at which collateral becomes less than maintenanceMargin
    pub liquidation_price: f64,

    /// can be cross or isolated
    pub margin_mode: MarginMode,

    /// margin ratio
    pub margin_ratio: f64,

    /// represents unrealizedPnl / initialMargin * 100
    pub percentage: f64,
}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PositionSide {
    Long,
    Short,
}

impl Default for PositionSide {
    fn default() -> Self {
        PositionSide::Long
    }
}


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BorrowInterest {
    /// The market that the interest was accrued in. None for cross margin.
    pub market: Option<Market>,

    /// The currency of the interest
    pub currency: String,

    /// The amount of interest that was charged
    pub interest: f64,

    /// The borrow interest rate
    pub interest_rate: f64,

    /// The amount of currency that was borrowed
    pub amount_borrowed: f64,

    /// The timestamp that the interest was charged
    pub timestamp: i64,
}


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Order {
    ///  The string ID of the order within the exchange.
    pub id: Option<String>,

    ///  a user-defined clientOrderId, if any
    pub client_order_id: Option<String>,

    /// order placing/opening Unix timestamp in milliseconds
    pub timestamp: i64,

    /// Unix timestamp of the most recent trade on this order
    pub last_trade_timestamp: Option<i64>,

    /// 'open', 'closed', 'canceled', 'expired', 'rejected'
    pub status: OrderStatus,

    /// market
    pub market: Market,

    /// 'market', 'limit', 'market by', 'trigger', 'stop loss', 'take profit',
    pub order_type: OrderType,

    /// 'GTC', 'IOC', 'FOK', 'PO'
    pub time_in_force: Option<TimeInForce>,

    /// 'buy', 'sell'
    pub side: Option<OrderSide>,

    /// float price in quote currency (may be empty for market orders)
    pub price: Option<f64>,

    /// float average filling price
    pub average: Option<f64>,

    /// ordered amount of base currency
    pub amount: f64,

    /// filled amount of base currency
    pub filled: Option<f64>,

    /// remaining amount to fill
    pub remaining: Option<f64>,

    /// 'filled' * 'price' (filling price used where available)
    pub cost: Option<f64>,

    /// a list of order trades/executions
    pub trades: Option<Vec<Trade>>,

    /// fee info, if available
    pub fee: Option<OrderFee>,

    pub margin_mode: MarginMode,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Trade {
    /// trade id
    pub id: String,

    /// Unix timestamp in milliseconds
    pub timestamp: i64,

    /// ISO8601 datetime with milliseconds
    pub datetime: String,

    /// market
    pub market: Market,

    /// string order id or None
    pub order_id: Option<String>,

    /// order type, 'market', 'limit' or undefined/None/null
    pub order_type: Option<OrderType>,

    /// direction of the trade, 'buy' or 'sell'
    pub side: Option<OrderSide>,

    /// taker or maker
    pub is_maker: bool,

    /// float price in quote currency
    pub price: f64,

    /// amount of base currency
    pub amount: f64,

    /// total cost, `price * amount`,
    pub cost: f64,

    /// provided by exchange or calculated by ccxtr
    pub fee: Option<OrderFee>,

    /// an array of fees if paid in multiple currencies
    pub fees: Option<Vec<OrderFee>>,
}


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OrderFee {
    /// which currency the fee is (usually quote)
    currency: String,

    /// the fee amount in that currency
    cost: f64,

    /// the fee rate (if available)
    rate: Option<f64>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Buy => OrderSide::Sell,
            OrderSide::Sell => OrderSide::Buy,
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "buy" | "BUY" => Some(OrderSide::Buy),
            "sell" | "SELL" => Some(OrderSide::Sell),
            _ => None,
        }
    }
}

impl Into<String> for OrderSide {
    fn into(self) -> String {
        match self {
            OrderSide::Buy => "buy".to_string(),
            OrderSide::Sell => "sell".to_string(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TimeInForce {
    /// Good Till Cancel(ed), the order stays on the orderbook until it is matched or canceled.
    GTC,

    /// Immediate Or Cancel, the order has to be matched immediately and filled either partially
    /// or completely, the unfilled remainder is canceled (or the entire order is canceled).
    IOC,

    /// Fill Or Kill, the order has to get fully filled and closed immediately, otherwise the entire
    /// order is canceled.
    FOK,

    /// Post Only, the order is either placed as a maker order, or it is canceled. This means the
    /// order must be placed on orderbook for at at least time in an unfilled state. The unification
    /// of PO as a timeInForce option is a work in progress with unified exchanges having
    /// exchange.has['postOnly'] == True.
    PO,
}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MarginMode {
    Cross,
    Isolated,
}

impl Default for MarginMode {
    fn default() -> Self {
        MarginMode::Isolated
    }
}


#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderType {
    /// regular orders having an amount in base currency (how much you want to buy or sell) and a
    /// price in quote currency (for which price you want to buy or sell).
    Limit,
    /// regular orders having an amount in base currency (how much you want to buy or sell)
    Market,
    /// some exchanges require market buy orders with an amount in quote currency (how much you want to spend for buying)
    MarketBuy,
    /// an advanced type of order used to wait for a certain condition on a market and then react
    /// automatically: when a triggerPrice is reached, the trigger order gets triggered and then a
    /// regular limit price or market price order is placed, that eventually results in entering a
    /// position or exiting a position
    Trigger,
    /// almost the same as trigger orders, but used to close a position to stop further losses on
    /// that position: when the price eaches triggerPrice then the stop loss order is triggered that
    /// results in placing another regular limit or market order to close a position at a specific
    /// limit price or at market price (a position with a stop loss order attached to it).
    StopLoss,
    /// a counterpart to stop loss orders, this type of order is used to close a position to take
    /// existing profits on that position: when the price reaches triggerPrice then the take profit
    /// order is triggered that results in placing another regular limit or market order to close a
    /// position at a specific limit price or at market price (a position with a take profit order
    /// attached to it).
    TakeProfit,
}

impl Display for OrderType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self {
            OrderType::Limit => write!(f, "LIMIT"),
            OrderType::Market => write!(f, "MARKET"),
            OrderType::MarketBuy => write!(f, "MARKET BUY"),
            OrderType::Trigger => write!(f, "TRIGGER"),
            OrderType::StopLoss => write!(f, "STOP LOSS"),
            OrderType::TakeProfit => write!(f, "TAKE PROFIT"),
        }
    }
}

impl Default for OrderType {
    fn default() -> Self {
        OrderType::Limit
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderStatus {
    Open,
    Closed,
    Canceled,
    Expired,
    Rejected,
    Unknown,
}

impl Default for OrderStatus {
    fn default() -> Self {
        OrderStatus::Unknown
    }
}


#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct OrderBook {
    pub bids: Vec<OrderBookUnit>,
    pub asks: Vec<OrderBookUnit>,
    pub market: Market,
    pub timestamp: Option<i64>,
    pub last_update_id: Option<i64>,
}

impl Default for OrderBook {
    fn default() -> Self {
        Self {
            bids: Vec::new(),
            asks: Vec::new(),
            market: Market::default(),
            timestamp: None,
            last_update_id: None,
        }
    }
}

impl OrderBook {
    pub fn new(bids: Vec<OrderBookUnit>, asks: Vec<OrderBookUnit>, market: Market, timestamp: Option<i64>, last_update_id: Option<i64>) -> Self {
        Self {
            bids,
            asks,
            market,
            timestamp,
            last_update_id,
        }
    }
}

impl From<String> for OrderBook {
    fn from(value: String) -> Self {
        serde_json::from_str(&value).unwrap()
    }
}


#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OrderBookUnit {
    pub price: f64,
    pub amount: f64,
}

impl Into<(f64, f64)> for OrderBookUnit {
    fn into(self) -> (f64, f64) {
        (self.price, self.amount)
    }
}

impl From<(f64, f64)> for OrderBookUnit {
    fn from(value: (f64, f64)) -> Self {
        Self {
            price: value.0,
            amount: value.1,
        }
    }
}


impl TryFrom<&Vec<String>> for OrderBookUnit {
    type Error = OrderBookError;

    fn try_from(value: &Vec<String>) -> OrderBookResult<Self> {
        Ok(OrderBookUnit {
            price: value[0].parse::<f64>()?,
            amount: value[1].parse::<f64>()?,
        })
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MarginLoan {
    /// the transaction id
    pub id: String,

    /// the currency that is borrowed or repaid
    pub currency: String,

    /// the amount of currency that was borrowed or repaid
    pub amount: f64,

    /// unified market
    pub market: Market,

    /// the timestamp of when the transaction was made
    pub timestamp: i64,
}


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Balance {
    /// Unix Timestamp in milliseconds
    pub timestamp: Option<i64>,

    /// the list of balance items
    pub items: Vec<BalanceItem>,
}


#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BalanceItem {
    /// currency code
    pub currency: String,

    /// market for isolated margin
    pub market: Option<Market>,

    /// money available for trading
    pub free: f64,

    /// money on hold, locked, frozen or pending
    pub used: f64,

    /// total balance (free + used)
    pub total: f64,

    /// debt
    pub debt: f64,

}


#[derive(Serialize, Deserialize, Debug, Default)]
#[non_exhaustive]
pub struct Ticker {
    pub ask: Option<f64>,
    #[serde(rename = "askVolume")]
    pub ask_volume: f64,
    pub average: f64,
    #[serde(rename = "baseVolume")]
    pub base_volume: f64,
    pub bid: Option<f64>,
    #[serde(rename = "bidVolume")]
    pub bid_volume: f64,
    pub change: f64,
    pub close: f64,
    pub high: f64,
    pub last: f64,
    pub low: f64,
    pub open: f64,
    pub percentage: f64,
    #[serde(rename = "previousClose")]
    pub previous_close: Option<f64>,
    #[serde(rename = "quoteVolume")]
    pub quote_volume: f64,
    pub market: Market,
    pub timestamp: i64,
    pub vwap: f64,
}