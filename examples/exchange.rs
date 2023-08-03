use std::thread::sleep;

use futures::StreamExt;

use ccxtr::{BinanceMargin, PropertiesBuilder};
use ccxtr::Exchange;
use ccxtr::model::{Market, MarketType, Order, OrderSide, OrderType};

#[tokio::main]
async fn main() {
    let api_key = std::env::var("API_KEY").unwrap();
    let secret = std::env::var("SECRET").unwrap();
    let props = PropertiesBuilder::new().api_key(api_key).secret(secret).build();
    let mut ex = BinanceMargin::new(props).unwrap();
    ex.fetch_markets().await.unwrap();
    ex.connect().await.unwrap();
    let markets = ex.load_markets().await.unwrap();
    let mut btc_usdt: Option<Market> = None;
    for m in markets {
        match m {
            Market { ref base, ref quote, ref market_type, .. } if base == "BTC" && quote == "USDT" && *market_type == MarketType::Margin => {
                btc_usdt = Some(m.clone());
                break;
            }
            _ => (),
        }
    }
    let mut stream = ex.watch_order_book(&vec![btc_usdt.as_ref().unwrap().clone()]).await.unwrap();
    tokio::spawn(async move {
        println!("start watching order book");
        while let Some(Ok(x)) = stream.next().await {
            println!("order book: {:?}", x)
        }
    });

    let order = Order {
        market: btc_usdt.unwrap().clone(),
        order_type: OrderType::Limit,
        side: OrderSide::Buy,
        price: Some(20000_f64),
        amount: 0.001,
        ..Default::default()
    };
    let _ = ex.create_order(order).await.or_else(|e| {
        println!("create order error: {:?}", e);
        Err(e)
    });
    sleep(std::time::Duration::from_secs(10));
}