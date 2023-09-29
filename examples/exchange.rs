use std::sync::{Arc, atomic};
use ccxtr::{BinanceMargin, ConnectError, OrderBookError, OrderBookResult, PropertiesBuilder};
use ccxtr::Exchange;
use ccxtr::model::{MarginType, Market, MarketType, Order, OrderSide, OrderType};

#[tokio::main]
async fn main() {
    let api_key = std::env::var("API_KEY").unwrap();
    let secret = std::env::var("SECRET").unwrap();
    let props = PropertiesBuilder::new().api_key(api_key.as_str()).secret(secret.as_str()).build();
    let mut ex = Arc::new(BinanceMargin::new(&props).unwrap());

    Arc::get_mut(&mut ex).unwrap().connect().await.unwrap();
    let markets = Arc::get_mut(&mut ex).unwrap().load_markets().await.unwrap();
    let mut subscriptions = Vec::new();
    let mut order_market = None;
    for m in markets {
        match m {
            Market { ref base, ref quote, ref market_type, .. } if quote == "BTC" && *market_type == MarketType::Margin => {
                subscriptions.push(m.clone());
            }
            Market { ref base, ref quote, ref market_type, .. } if base == "BTC" && quote == "USDT" && *market_type == MarketType::Margin => {
                order_market = Some(m.clone());
            }
            _ => (),
        }
    }

    // create_order(&mut ex, &order_market.unwrap()).await;
    let subscriptions = Arc::new(subscriptions[0..10].to_vec());
    println!("subscriptions: {:?}", subscriptions.len());
    let select = Arc::new(atomic::AtomicI64::new(0));
    let stream = ex.watch_order_book(&subscriptions).await;
    if stream.is_err() {
        println!("failed to watch order book: {:?}", stream.err().unwrap());
        return;
    }
    let stream = stream.unwrap();
    tokio::spawn({
        let select = select.clone();
        let selections = subscriptions.clone();
        async move {
            println!("start watching order book");
            while let Ok(result) = stream.receive().await {
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
                    }
                    Err(OrderBookError::SynchronizationError(m)) => {
                        println!("synchronization error: {:?}", m);
                        ex.watch_order_book(&vec![m]).await.unwrap();
                    }
                    Err(OrderBookError::InvalidOrderBook(_, m)) => {
                        panic!("invalid order book: {:?}", m);
                    }
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
            }
            "n" => {
                let mut num = select.load(atomic::Ordering::Relaxed);
                num += 1;
                if num >= subscriptions.len() as i64 {
                    num = 0;
                }
                println!("select: {}", subscriptions[num as usize]);
                select.store(num, atomic::Ordering::Relaxed);
            }
            "p" => {
                let mut num = select.load(atomic::Ordering::Relaxed);
                num -= 1;
                if num < 0 {
                    num = subscriptions.len() as i64 - 1;
                }
                println!("select: {}", subscriptions[num as usize]);
                select.store(num, atomic::Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

async fn create_order(ex: &mut BinanceMargin, order_market: &Market) {
    let order = Order {
        market: order_market.clone(),
        order_type: OrderType::Limit,
        side: Some(OrderSide::Buy),
        price: Some(20000_f64),
        amount: 0.001,
        margin_type: MarginType::MarginBuy,
        ..Default::default()
    };
    let order = ex.create_order(order).await.or_else(|e| {
        println!("create order error: {:?}", e);
        Err(e)
    });
    println!("order: {:?}", order);
}