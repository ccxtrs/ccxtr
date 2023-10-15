use std::num::ParseFloatError;
use std::sync::PoisonError;

use hmac::digest::InvalidLength;
use thiserror::Error;

use crate::exchange::BasePropertiesBuilderError;
use crate::model::Market;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq)]
pub(crate) enum Error {
    NotImplemented,
    MarketNotInitialized,
    InvalidTimestamp(i64),
    LockError(String),
    DeserializeJsonBody(String),
    HttpError(String),
    WebsocketError(String),
    MissingField(String),
    ParseError(String),
    MissingProperties(String),
    SymbolNotFound(String),
    InvalidPrice(String),
    InvalidMarket,
    InvalidQuantity(String),
    InvalidCredentials,
    InvalidParameters(String),
    InvalidSignature(String),
    MissingMarkets,

    UnsupportedOrderType(String),
    UnsupportedOrderSide(String),
    UnsupportedOrderStatus(String),
    UnsupportedTimeInForce(String),
    CredentialsError(String),

    InvalidOrderBook(String),
    InvalidResponse(String),
    SynchronizationError,

    InsufficientMargin(String),
}


impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::HttpError(format!("{}", e))
    }
}

impl From<ParseFloatError> for Error {
    fn from(e: ParseFloatError) -> Self {
        Error::ParseError(format!("{}", e))
    }
}

impl From<InvalidLength> for Error {
    fn from(value: InvalidLength) -> Self {
        Error::CredentialsError(format!("{}", value))
    }
}


impl<T> From<PoisonError<T>> for Error {
    fn from(value: PoisonError<T>) -> Self {
        Error::LockError(format!("{:?}", value))
    }
}


pub type CommonResult<T> = std::result::Result<T, CommonError>;

#[derive(Error, Debug)]
pub enum CommonError {
    #[error("not implemented")]
    NotImplemented,
    #[error("connection error {0}")]
    ConnectionError(String),
    #[error("lock error {0}")]
    LockError(String),
    #[error("deserialization error for json body {0}")]
    DeserializeJsonBody(String),
    #[error("http error {0}")]
    HttpError(String),
    #[error("websocket error {0}")]
    WebsocketError(String),
    #[error("missing field {0}")]
    MissingField(String),
    #[error("parse error {0}")]
    ParseError(String),
    #[error("missing properties {0}")]
    MissingProperties(String),
    #[error("symbol not found {0}")]
    SymbolNotFound(String),
    #[error("missing credentials")]
    MissingCredentials,
    #[error("fetch markets first")]
    MissingMarkets,

    #[error("unsupported order type {0}")]
    UnsupportedOrderType(String),
    #[error("unsupported order side {0}")]
    UnsupportedOrderSide(String),
    #[error("unsupported order status {0}")]
    UnsupportedOrderStatus(String),
    #[error("unsupported time in force {0}")]
    UnsupportedTimeInForce(String),
    #[error("credentials error {0}")]
    CredentialsError(String),

    #[error("invalid order book {0}")]
    InvalidOrderBook(String),

    /// create order error
    #[error("insufficient margin {0}")]
    InsufficientMargin(String),
    #[error("invalid price {0}")]
    InvalidPrice(String),
    #[error("invalid market")]
    InvalidMarket,

    #[error("invalid timestamp {0}")]
    InvalidTimestamp(i64),

    #[error("parse error {0}")]
    ParseFloatError(#[from] ParseFloatError),
}

impl From<BasePropertiesBuilderError> for CommonError {
    fn from(value: BasePropertiesBuilderError) -> Self {
        CommonError::MissingProperties(value.to_string())
    }
}

impl From<Error> for CommonError {
    fn from(err: Error) -> Self {
        match err {
            Error::NotImplemented => CommonError::NotImplemented,
            Error::LockError(e) => CommonError::LockError(e),
            Error::DeserializeJsonBody(e) => CommonError::DeserializeJsonBody(e),
            Error::HttpError(e) => CommonError::HttpError(e),
            Error::WebsocketError(e) => CommonError::WebsocketError(e),
            Error::MissingField(e) => CommonError::MissingField(e),
            Error::ParseError(e) => CommonError::ParseError(e),
            Error::MissingProperties(e) => CommonError::MissingProperties(e),
            Error::SymbolNotFound(e) => CommonError::SymbolNotFound(e),
            Error::InvalidCredentials => CommonError::MissingCredentials,
            Error::MissingMarkets => CommonError::MissingMarkets,
            Error::UnsupportedOrderType(e) => CommonError::UnsupportedOrderType(e),
            Error::UnsupportedOrderSide(e) => CommonError::UnsupportedOrderSide(e),
            Error::UnsupportedOrderStatus(e) => CommonError::UnsupportedOrderStatus(e),
            Error::UnsupportedTimeInForce(e) => CommonError::UnsupportedTimeInForce(e),
            Error::CredentialsError(e) => CommonError::CredentialsError(e),
            Error::InvalidOrderBook(e) => CommonError::InvalidOrderBook(e),
            Error::InsufficientMargin(e) => CommonError::InsufficientMargin(e),
            Error::InvalidPrice(e) => CommonError::InvalidPrice(e),
            Error::InvalidQuantity(e) => CommonError::InvalidPrice(e),
            Error::InvalidSignature(e) => CommonError::InvalidPrice(e),
            Error::InvalidParameters(e) => CommonError::InvalidPrice(e),
            Error::InvalidMarket => CommonError::InvalidMarket,
            Error::InvalidTimestamp(ts) => CommonError::InvalidTimestamp(ts),
            _ => CommonError::NotImplemented,
        }
    }
}

pub type ConnectResult<T> = std::result::Result<T, ConnectError>;

#[derive(Error, Debug)]
pub enum ConnectError {
    #[error("not implemented")]
    NotImplemented,
    #[error("load market error {0}")]
    LoadMarketError(#[from] LoadMarketError),
    #[error("unknown error {0}")]
    UnknownError(String),
}


impl From<Error> for ConnectError {
    fn from(err: Error) -> Self {
        match err {
            _ => ConnectError::UnknownError(format!("{:?}", err)),
        }
    }
}


pub type OrderBookResult<T> = std::result::Result<T, OrderBookError>;

#[derive(Error, Debug, Clone)]
pub enum OrderBookError {
    #[error("not implemented")]
    NotImplemented,
    #[error("websocket error {0}")]
    WebsocketError(String),
    #[error("invalid order book {0}")]
    InvalidOrderBook(String, Option<Market>),
    #[error("parse error {0}")]
    ParseError(String),
    #[error("synchronization error {0}")]
    SynchronizationError(Market),
    #[error("unknown error {0}")]
    UnknownError(String),
}


impl From<Error> for OrderBookError {
    fn from(err: Error) -> Self {
        match err {
            _ => OrderBookError::UnknownError(format!("{:?}", err)),
        }
    }
}

impl From<ParseFloatError> for OrderBookError {
    fn from(e: ParseFloatError) -> Self {
        OrderBookError::ParseError(format!("{}", e))
    }
}


pub type WatchResult<T> = std::result::Result<T, WatchError>;

#[derive(Error, Debug)]
pub enum WatchError {
    #[error("not implemented")]
    NotImplemented,
    #[error("websocket error {0}")]
    WebsocketError(String),
    #[error("symbol not found {0}")]
    SymbolNotFound(String),
    #[error("not connected")]
    NotConnected,
    #[error("receiving skipped {0} messages")]
    Overflowed(u64),
    #[error("unknown error {0}")]
    UnknownError(String),
}


impl From<Error> for WatchError {
    fn from(err: Error) -> Self {
        match err {
            _ => WatchError::UnknownError(format!("{:?}", err)),
        }
    }
}


impl From<async_broadcast::RecvError> for WatchError {
    fn from(e: async_broadcast::RecvError) -> Self {
        match e {
            async_broadcast::RecvError::Overflowed(n) => WatchError::Overflowed(n),
            async_broadcast::RecvError::Closed => WatchError::NotConnected,
        }
    }
}

impl<T> From<async_broadcast::SendError<T>> for WatchError {
    fn from(e: async_broadcast::SendError<T>) -> Self {
        WatchError::WebsocketError(format!("{}", e))
    }
}

impl From<flume::RecvError> for WatchError {
    fn from(e: flume::RecvError) -> Self {
        WatchError::UnknownError(e.to_string())
    }
}

impl<T> From<flume::SendError<T>> for WatchError {
    fn from(e: flume::SendError<T>) -> Self {
        WatchError::WebsocketError(format!("{}", e))
    }
}


pub type CreateOrderResult<T> = std::result::Result<T, CreateOrderError>;

#[derive(Error, Debug)]
pub enum CreateOrderError {
    #[error("insufficient margin {0}")]
    InsufficientMargin(String),
    #[error("invalid price {0}")]
    InvalidPrice(String),
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unsupported order type {0}")]
    UnsupportedOrderType(String),
    #[error("symbol not found {0}")]
    SymbolNotFound(String),
    #[error("not implemented")]
    NotImplemented,
    #[error("unknown error {0}")]
    UnknownError(String),
}

impl From<Error> for CreateOrderError {
    fn from(e: Error) -> Self {
        match e {
            Error::InsufficientMargin(s) => CreateOrderError::InsufficientMargin(s),
            Error::InvalidPrice(s) => CreateOrderError::InvalidPrice(s),
            Error::InvalidCredentials => CreateOrderError::InvalidCredentials,
            Error::UnsupportedOrderType(s) => CreateOrderError::UnsupportedOrderType(s),
            Error::SymbolNotFound(s) => CreateOrderError::SymbolNotFound(s),
            Error::NotImplemented => CreateOrderError::NotImplemented,
            _ => CreateOrderError::UnknownError(format!("{:?}", e)),
        }
    }
}

pub type LoadMarketResult<T> = std::result::Result<T, LoadMarketError>;

#[derive(Error, Debug)]
pub enum LoadMarketError {
    #[error("not implemented")]
    NotImplemented,
    #[error("unknown error {0}")]
    UnknownError(String),
}

impl From<Error> for LoadMarketError {
    fn from(e: Error) -> Self {
        match e {
            _ => LoadMarketError::UnknownError(format!("{:?}", e)),
        }
    }
}


pub type FetchMarketResult<T> = std::result::Result<T, FetchMarketError>;

#[derive(Error, Debug)]
pub enum FetchMarketError {
    #[error("not implemented")]
    NotImplemented,
    #[error("unknown error {0}")]
    UnknownError(String),
}

impl From<Error> for FetchMarketError {
    fn from(e: Error) -> Self {
        match e {
            _ => FetchMarketError::UnknownError(format!("{:?}", e)),
        }
    }
}

impl From<FetchMarketError> for LoadMarketError {
    fn from(value: FetchMarketError) -> Self {
        match value {
            FetchMarketError::NotImplemented => LoadMarketError::NotImplemented,
            FetchMarketError::UnknownError(e) => LoadMarketError::UnknownError(e),
        }
    }
}


pub type FetchBalanceResult<T> = std::result::Result<T, FetchBalanceError>;

#[derive(Error, Debug)]
pub enum FetchBalanceError {
    #[error("not implemented")]
    NotImplemented,
    #[error("unknown error {0}")]
    UnknownError(String),
}


impl From<Error> for FetchBalanceError {
    fn from(e: Error) -> Self {
        match e {
            _ => FetchBalanceError::UnknownError(format!("{:?}", e)),
        }
    }
}

pub type FetchPositionsResult<T> = std::result::Result<T, FetchPositionsError>;

#[derive(Error, Debug)]
pub enum FetchPositionsError {
    #[error("not implemented")]
    NotImplemented,
    #[error("unknown error {0}")]
    UnknownError(String),
}


impl From<Error> for FetchPositionsError {
    fn from(e: Error) -> Self {
        match e {
            _ => FetchPositionsError::UnknownError(format!("{:?}", e)),
        }
    }
}

