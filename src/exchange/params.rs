use derive_builder::Builder;

use crate::model::{MarginMode, Market, OrderSide, OrderType, TimeInForce};

#[derive(Default, Builder, Debug)]
#[builder(default)]
pub struct FetchBalanceParams {
    pub margin_mode: Option<MarginMode>,
}



#[derive(Default, Builder, Debug)]
#[builder(default)]
pub struct FetchPositionsParams {
}



#[derive(Builder, Debug)]
#[builder(default)]
pub struct CreateOrderParams {
    pub market: Market,
    pub price: Option<f64>,
    pub amount: f64,
    pub order_side: OrderSide,
    pub order_type: Option<OrderType>,
    pub margin_mode: Option<MarginMode>,
    pub time_in_force: Option<TimeInForce>,
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
            time_in_force: None,
            reduce_only: false,
        }
    }
}