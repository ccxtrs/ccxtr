use std::hash::{Hash, Hasher};
use std::ops;

use serde::{Deserialize, Serialize};

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
    Base,
    Quote,
    Other
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketFee {
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
    /// The string ID of the currency within the exchange. Currency ids are used inside
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
    /// The string ID of the network within the exchange. Network ids are used inside
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
pub struct Order {
    ///  The string ID of the network within the exchange.
    pub id: String,

    ///  a user-defined clientOrderId, if any
    pub client_order_id: Option<String>,

    /// ISO8601 datetime of 'timestamp' with milliseconds
    pub datetime: String,

    /// order placing/opening Unix timestamp in milliseconds
    pub timestamp: i64,

    /// Unix timestamp of the most recent trade on this order
    pub last_trade_timestamp: Option<i64>,

    /// 'open', 'closed', 'canceled', 'expired', 'rejected'
    pub status: OrderStatus,

    /// symbol
    pub symbol: String,

    /// 'market', 'limit', 'market by', 'trigger', 'stop loss', 'take profit',
    pub order_type: OrderType,

    /// 'GTC', 'IOC', 'FOK', 'PO'
    pub time_in_force: Option<TimeInForce>,

    /// 'buy', 'sell'
    pub side: OrderSide,

    /// float price in quote currency (may be empty for market orders)
    pub price: Option<Decimal>,

    /// float average filling price
    pub average: Option<Decimal>,

    /// ordered amount of base currency
    pub amount: Decimal,

    /// filled amount of base currency
    pub filled: Option<Decimal>,

    /// remaining amount to fill
    pub remaining: Option<Decimal>,

    /// 'filled' * 'price' (filling price used where available)
    pub cost: Option<Decimal>,

    /// a list of order trades/executions
    pub trades: Option<Vec<Trade>>,

    /// fee info, if available
    pub fee: Option<OrderFee>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// trade id
    pub id: String,

    /// Unix timestamp in milliseconds
    pub timestamp: i64,

    /// ISO8601 datetime with milliseconds
    pub datetime: String,

    /// symbol
    pub symbol: String,

    /// string order id or None
    pub order_id: Option<String>,

    /// order type, 'market', 'limit' or undefined/None/null
    pub order_type: Option<OrderType>,

    /// direction of the trade, 'buy' or 'sell'
    pub side: OrderSide,

    /// taker or maker
    pub is_maker: bool,

    /// float price in quote currency
    pub price: Decimal,

    /// amount of base currency
    pub amount: Decimal,

    /// total cost, `price * amount`,
    pub cost: Decimal,

    /// provided by exchange or calculated by ccxtr
    pub fee: Option<OrderFee>,

    /// an array of fees if paid in multiple currencies
    pub fees: Option<Vec<OrderFee>>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFee {
    /// which currency the fee is (usually quote)
    currency: String,

    /// the fee amount in that currency
    cost: Decimal,

    /// the fee rate (if available)
    rate: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    Open,
    Closed,
    Canceled,
    Expired,
    Rejected,
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
    pub fn new(bids: Vec<OrderBookUnit>, asks: Vec<OrderBookUnit>, symbol: String, timestamp: i64, datetime: String, nonce: Option<i64>) -> Self {
        Self{
            bids,
            asks,
            symbol,
            timestamp,
            datetime,
            nonce
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