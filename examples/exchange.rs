use std::thread::sleep;
use ccxtr::{BinanceUsdm, PropertiesBuilder};
use ccxtr::Exchange;
use ccxtr::model::{Decimal, Market, MarketType, Order, OrderSide, OrderType};
use futures::StreamExt;
use rust_decimal::prelude::FromPrimitive;

#[tokio::main]
async fn main() {
    let api_key = std::env::var("API_KEY").unwrap();
    let secret = std::env::var("SECRET").unwrap();
    let props = PropertiesBuilder::new().api_key(api_key).secret(secret).build();
    let mut ex = BinanceUsdm::new(props).unwrap();
    ex.fetch_markets().await.unwrap();
    ex.connect().await.unwrap();
    let markets = ex.load_markets().await.unwrap();
    let mut btc_usdt: Option<Market> = None;
    for m in markets {
        match m {
            Market{ref base, ref quote, ref market_type, ..} if base == "BTC" && quote == "USDT" && *market_type == MarketType::Swap => {
                btc_usdt = Some(m.clone());
                break;
            }
            _ => (),
        }
    }
    let mut stream = ex.watch_order_book(&vec![btc_usdt.as_ref().unwrap().clone()]).await.unwrap();
    tokio::spawn(async move {
        while let Some(Ok(x)) = stream.next().await {
            print!("best ask: ({:?}, {:?}) best bid: ({:?}, {:?})\n", x.asks[0].price, x.asks[0].amount, x.bids[0].price, x.bids[0].amount);
        }
    });

    let order = Order {
        market: btc_usdt.unwrap().clone(),
        order_type: OrderType::Limit,
        side: OrderSide::Buy,
        price: Some(Decimal::from(20000)),
        amount: Decimal::from_f64(0.001).unwrap(),
        ..Default::default()
    };
    let order = ex.create_order(order).await.or_else(|e| {
        println!("create order error: {:?}", e);
        Err(e)
    });
    sleep(std::time::Duration::from_secs(1));
}