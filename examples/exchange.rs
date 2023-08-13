use std::sync::{Arc, atomic};
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
    let selections = Arc::new(subscriptions[0..10].to_vec());
    let mut select = Arc::new(atomic::AtomicI64::new(0));
    let mut stream = ex.watch_order_book(&subscriptions).await.unwrap();
    tokio::spawn({
        let select = select.clone();
        let selections = selections.clone();
        async move {
            println!("start watching order book");
            while let Some(result) = stream.next().await {
                match result {
                    Ok(order_book) => {
                        if order_book.market == selections[select.load(atomic::Ordering::Relaxed) as usize] {
                            println!("[{}] bid={:?}({:?}) ask={:?}({:?})",
                                order_book.market,
                                order_book.bids[0].price,
                                order_book.bids[0].quantity,
                                order_book.asks[0].price,
                                order_book.asks[0].quantity,
                            );
                        }
                    },
                    Err(OrderBookError::InvalidOrderBook(_, m)) => {
                        let market = m.unwrap();
                        let _ = ex.watch_order_book(&vec![market.clone()]).await;
                    },
                    _ => {}
                }
            }
        }
    });

    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        match input {
            "q" => {
                break;
            },
            "n" => {
                let mut num = select.load(atomic::Ordering::Relaxed);
                num += 1;
                if num >= selections.len() as i64 {
                    num = 0;
                }
                println!("select: {}", selections[num as usize]);
                select.store(num, atomic::Ordering::Relaxed);
            },
            "p" => {
                let mut num = select.load(atomic::Ordering::Relaxed);
                num -= 1;
                if num < 0 {
                    num = selections.len() as i64 - 1;
                }
                println!("select: {}", selections[num as usize]);
                select.store(num, atomic::Ordering::Relaxed);
            },
            _ => {}
        }
    }

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

}