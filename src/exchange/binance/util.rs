use crate::Error::{UnsupportedOrderSide, UnsupportedOrderStatus, UnsupportedOrderType, UnsupportedTimeInForce};
use crate::model::{OrderSide, OrderStatus, OrderType, TimeInForce};
use crate::Result;

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
        _ => Err(UnsupportedTimeInForce(time_in_force.to_string())),
    }
}

pub(in super) fn get_exchange_time_in_force(time_in_force: &TimeInForce) -> Result<String> {
    match time_in_force {
        TimeInForce::GTC => Ok("GTC".to_string()),
        TimeInForce::IOC => Ok("IOC".to_string()),
        TimeInForce::FOK => Ok("FOK".to_string()),
        TimeInForce::PO => Ok("PO".to_string()),
    }
}



pub(in super) fn get_exchange_order_type(order_type: &OrderType) -> Result<String> {
    match order_type {
        OrderType::Limit => Ok("LIMIT".to_string()),
        OrderType::Market => Ok("MARKET".to_string()),
        OrderType::StopLoss => Ok("STOP_LOSS".to_string()),
        OrderType::TakeProfit => Ok("TAKE_PROFIT".to_string()),
        _ => Err(UnsupportedOrderType(order_type.to_string())),
    }
}

pub(in super) fn get_exchange_order_side(order_side: &OrderSide) -> String {
    match order_side {
        OrderSide::Buy => "BUY",
        OrderSide::Sell => "SELL",
    }.to_string()
}

pub(in super) fn get_unified_order_side(exchange_order_side: &str) -> Result<OrderSide> {
    match exchange_order_side {
        "BUY" => Ok(OrderSide::Buy),
        "SELL" => Ok(OrderSide::Sell),
        _ => Err(UnsupportedOrderSide(exchange_order_side.to_string())),
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
        _ => Err(UnsupportedOrderStatus(exchange_order_status.to_string())),
    }
}