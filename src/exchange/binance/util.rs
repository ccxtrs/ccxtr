use crate::error::{Error, Result};
use crate::model::{OrderSide, OrderStatus, OrderType, TimeInForce, WorkingType};

pub(in super) fn to_unified_asset(exchange_asset: &str) -> String {
    exchange_asset.to_uppercase()
}

pub(in super) fn is_active(status: Option<String>) -> bool {
    status.map(|status| status == "TRADING").unwrap_or(false)
}

pub(in super) fn get_unified_time_in_force(time_in_force: &str) -> Result<TimeInForce> {
    match time_in_force {
        "GTC" => Ok(TimeInForce::GTC),
        "IOC" => Ok(TimeInForce::IOC),
        "FOK" => Ok(TimeInForce::FOK),
        "PO" => Ok(TimeInForce::PO),
        _ => Err(Error::UnsupportedTimeInForce(time_in_force.to_string())),
    }
}

const GTC: &str = "GTC";
const IOC: &str = "IOC";
const FOK: &str = "FOK";
const PO: &str = "PO";

pub(in super) fn get_exchange_time_in_force(time_in_force: &TimeInForce) -> &'static str {
    match time_in_force {
        TimeInForce::GTC => GTC,
        TimeInForce::IOC => IOC,
        TimeInForce::FOK => FOK,
        TimeInForce::PO => PO,
    }
}


pub(in super) fn get_exchange_working_type(working_type: &WorkingType) -> Result<&'static str> {
    match working_type {
        WorkingType::MarkPrice => Ok("MARK_PRICE"),
        WorkingType::ContractPrice => Ok("CONTRACT_PRICE"),
        _ => Err(Error::UnsupportedWorkingType(working_type.to_string())),

    }
}

pub(in super) fn get_exchange_order_type(order_type: &OrderType) -> Result<&'static str> {
    match order_type {
        OrderType::Limit => Ok("LIMIT"),
        OrderType::Market => Ok("MARKET"),
        OrderType::StopLoss => Ok("STOP_LOSS"),
        OrderType::TakeProfit => Ok("TAKE_PROFIT"),
        OrderType::TrailingStopMarket => Ok("TRAILING_STOP_MARKET"),
        _ => Err(Error::UnsupportedOrderType(order_type.to_string())),
    }
}


const BUY: &str = "BUY";
const SELL: &str = "SELL";

pub(in super) fn get_exchange_order_side(order_side: &OrderSide) -> &'static str {
    match order_side {
        OrderSide::Buy => BUY,
        OrderSide::Sell => SELL,
    }
}

pub(in super) fn get_unified_order_side(exchange_order_side: &str) -> Result<OrderSide> {
    match exchange_order_side {
        "BUY" => Ok(OrderSide::Buy),
        "SELL" => Ok(OrderSide::Sell),
        _ => Err(Error::UnsupportedOrderSide(exchange_order_side.to_string())),
    }
}

pub(in super) fn get_unified_order_status(exchange_order_status: &str) -> Result<OrderStatus> {
    match exchange_order_status {
        "NEW" => Ok(OrderStatus::Open),
        "PARTIALLY_FILLED" => Ok(OrderStatus::Open),
        "FILLED" => Ok(OrderStatus::Closed),
        "CANCELED" => Ok(OrderStatus::Canceled),
        "PENDING_CANCEL" => Ok(OrderStatus::Canceled),
        "REJECTED" => Ok(OrderStatus::Rejected),
        "EXPIRED" => Ok(OrderStatus::Expired),
        _ => Err(Error::UnsupportedOrderStatus(exchange_order_status.to_string())),
    }
}