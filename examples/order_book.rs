use std::sync::Arc;
use ccxtr::{Binance, BinanceUsdm, Exchange, PropertiesBuilder, StreamItem, WatchOrderBookParamsBuilder, WatchResult};

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::default().channel_capacity(Some(5)).build().expect("failed to build properties");
    let mut ex = Arc::new(Binance::new(&props).unwrap());
    let markets = Arc::get_mut(&mut ex).unwrap().load_markets().await.unwrap();
    let markets = markets.into_iter().filter(|m| m.quote == "USDT" && m.base == "BTC").collect::<Vec<_>>();
    let params = WatchOrderBookParamsBuilder::default().markets(markets.clone()).build().expect("failed to build params");
    let mut stream = ex.watch_order_book(&params).await.expect("failed to watch order book");


    loop {
        match stream.receive().await {
            Ok(Some(StreamItem::OrderBook(order_book))) => {
                println!("ob: {:?}", order_book);
            }
            _ => {}
        }
    }

}