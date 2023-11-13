use ccxtr::{Binance, BinanceUsdm, CreateOrderParamsBuilder, Exchange, PropertiesBuilder};
use ccxtr::model::{MarginMode, Market, OrderSide, OrderType};

#[tokio::main]
async fn main() {
    let api_key = std::env::var("API_KEY").expect("failed to get api key");
    let secret = std::env::var("SECRET").expect("failed to get secret key");
    let props = PropertiesBuilder::default().api_key(Some(api_key)).secret(Some(secret)).build().expect("failed to build properties");
    let mut ex = BinanceUsdm::new(props).expect("failed to create exchange");
    let markets = ex.load_markets().await.expect("failed to load markets");
    let target_market = markets.into_iter().find(|m| m.quote == "USDT" && m.base == "GAS").unwrap();
    create_order(&mut ex, &target_market).await;
}


async fn create_order(ex: &mut BinanceUsdm, order_market: &Market) {
    let params = CreateOrderParamsBuilder::default()
        .market(order_market.clone())
        .amount(0.6)
        .margin_mode(Some(MarginMode::Cross))
        .order_type(Some(OrderType::TrailingStopMarket))
        .callback_rate(Some(3f64))
        .order_side(OrderSide::Sell)
        .build().expect("failed to build params");
    let order = ex.create_order(params).await.or_else(|e| {
        println!("create order error: {:?}", e);
        Err(e)
    });
    println!("order: {:?}", order);
}