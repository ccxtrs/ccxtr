use derive_builder::Builder;

use crate::model::{MarginMode, Market, OrderSide, OrderType, TimeInForce, WorkingType};

#[derive(Default, Builder, Debug)]
#[builder(default)]
#[non_exhaustive]
pub struct WatchOrderBookParams {
    pub markets: Vec<Market>,
}


#[derive(Default, Builder, Debug)]
#[builder(default)]
#[non_exhaustive]
pub struct FetchBalanceParams {
    pub margin_mode: Option<MarginMode>,
}


#[derive(Default, Builder, Debug)]
#[builder(default)]
#[non_exhaustive]
pub struct FetchTickersParams {
    pub markets: Option<Vec<Market>>,
    pub chunk_size: Option<usize>,
}


#[derive(Default, Builder, Debug)]
#[builder(default)]
#[non_exhaustive]
pub struct FetchPositionsParams {}


#[derive(Builder, Debug)]
#[builder(default)]
#[non_exhaustive]
pub struct CreateOrderParams {
    pub market: Market,
    pub price: Option<f64>,
    pub amount: f64,
    pub order_side: OrderSide,
    pub order_type: Option<OrderType>,
    pub margin_mode: Option<MarginMode>,
    pub time_in_force: Option<TimeInForce>,
    pub callback_rate: Option<f64>,
    pub working_type: Option<WorkingType>,
    pub reduce_only: bool,
}

impl Default for CreateOrderParams {
    fn default() -> Self {
        Self {
            market: Market::default(),
            price: None,
            amount: 0.0,
            order_side: OrderSide::Buy,
            order_type: None,
            margin_mode: None,
            callback_rate: None,
            working_type: None,
            time_in_force: None,
            reduce_only: false,
        }
    }
}