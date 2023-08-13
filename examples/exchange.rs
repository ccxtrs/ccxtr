use std::thread::sleep;

use futures::StreamExt;

use ccxtr::{BinanceMargin, OrderBookError, OrderBookResult, PropertiesBuilder};
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
    let mut subscriptions = Vec::new();
    for m in markets {
        match m {
            Market { ref base, ref quote, ref market_type, .. } if quote == "BTC" && *market_type == MarketType::Margin => {
                subscriptions.push(m.clone());
            }
            _ => (),
        }
    }
    // subscriptions = subscriptions[0..30].to_vec();
    println!("subscriptions: {:?}", subscriptions.len());
    let mut stream = ex.watch_order_book(&subscriptions).await.unwrap();
    tokio::spawn(async move {
        println!("start watching order book");
        let mut err_markets = vec![];
        while let Some(result) = stream.next().await {
            match result {
                Ok(order_book) => {
                    if err_markets.contains(&order_book.market) {
                        println!("recovered: {:?}", order_book.market);
                        err_markets.retain(|m| m != &order_book.market);
                    }
                },
                Err(OrderBookError::InvalidOrderBook(_, m)) => {
                    println!("invalid order book: {:?}", m);
                    let market = m.unwrap();
                    err_markets.push(market.clone());
                    let _ = ex.watch_order_book(&vec![market.clone()]).await;
                },
                _ => {}
            }
        }
    });

    // let order = Order {
    //     market: btc_usdt.unwrap().clone(),
    //     order_type: OrderType::Limit,
    //     side: OrderSide::Buy,
    //     price: Some(20000_f64),
    //     amount: 0.001,
    //     ..Default::default()
    // };
    // let _ = ex.create_order(order).await.or_else(|e| {
    //     println!("create order error: {:?}", e);
    //     Err(e)
    // });
    sleep(std::time::Duration::from_secs(100));

}