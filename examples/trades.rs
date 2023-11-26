use std::sync::Arc;
use ccxtr::{BinanceUsdm, Exchange, FetchTradesParams, FetchTradesParamsBuilder, PropertiesBuilder, StreamItem, WatchOrderBookParamsBuilder, WatchTradesParams, WatchTradesParamsBuilder};

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::default().channel_capacity(Some(5)).build().expect("failed to build properties");
    let mut ex = Arc::new(BinanceUsdm::new(props).unwrap());
    let markets = Arc::get_mut(&mut ex).unwrap().load_markets().await.unwrap();
    let market = markets.into_iter().filter(|m| m.base == "BTC" && m.quote == "USDT").collect::<Vec<_>>().first().expect("failed to get market").clone();


    let trades = ex.fetch_trades(FetchTradesParamsBuilder::default().market(market.clone()).build().expect("failed to build params")).await.expect("failed to fetch trades");
    println!("len: {}", trades.len());
    println!("trades: {:?}", trades.first().expect("failed to get trades"));


    let params = WatchTradesParamsBuilder::default().markets(vec![market.clone()]).build().expect("failed to build params");
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