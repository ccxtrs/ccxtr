use std::sync::Arc;
use ccxtr::{BinanceUsdm, Exchange, PropertiesBuilder};

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::default().channel_capacity(Some(5)).build().expect("failed to build properties");
    let mut ex = Arc::new(BinanceUsdm::new(&props).unwrap());
    let markets = Arc::get_mut(&mut ex).unwrap().load_markets().await.unwrap();
    let markets = markets.into_iter().filter(|m| m.quote == "USDT" && m.base == "BTC").collect::<Vec<_>>();
    let mut stream = ex.watch_order_book(&markets).await.expect("failed to watch order book");


    tokio::spawn({
        let mut stream = stream.clone();
        async move {
            loop {
                match stream.receive().await {
                    Ok(Ok(order_book)) => {
                        println!("[1] timestamp={:?} len={:?}", order_book.timestamp, stream.len());
                    }
                    Ok(Err(e)) => {
                        println!("[1] order book error={:?}", e);
                    }
                    Err(e) => {
                        println!("[1] watch error={:?}", e);
                    }
                }
            }
        }
    });


    tokio::spawn({
        async move {
            loop {
                match stream.receive().await {
                    Ok(Ok(order_book)) => {
                        println!("[2] timestamp={:?} len={:?}", order_book.timestamp, stream.len());
                    }
                    Ok(Err(e)) => {
                        println!("[2] order book error={:?}", e);
                    }
                    Err(e) => {
                        println!("[2] watch error={:?}", e);
                    }
                }
            }
        }
    });

    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
}