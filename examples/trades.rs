use std::sync::Arc;
use ccxtr::{BinanceUsdm, Exchange, PropertiesBuilder, StreamItem, WatchOrderBookParamsBuilder, WatchTradesParamsBuilder};

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::default().channel_capacity(Some(5)).build().expect("failed to build properties");
    let mut ex = Arc::new(BinanceUsdm::new(props).unwrap());
    let markets = Arc::get_mut(&mut ex).unwrap().load_markets().await.unwrap();
    let markets = markets.into_iter().filter(|m| m.base == "BTC" && m.quote == "USDT").collect::<Vec<_>>();
    println!("len: {}", markets.len());
    let params = WatchTradesParamsBuilder::default().markets(markets.clone()).build().expect("failed to build params");
    let mut stream = ex.watch_trades(params).await.expect("failed to watch order book");


    loop {
        match stream.receive().await {
            Ok(StreamItem::Trade(Ok(trade))) => {
                println!("trade: {:?}", trade);
            }
            other => {
                println!("other: {:?}", other);
            }
        }
    }

}