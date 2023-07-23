use ccxtr::{BinanceUsdm, PropertiesBuilder};
use ccxtr::Exchange;
use ccxtr::model::Market;
use futures::StreamExt;

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::new().build();
    let mut ex = BinanceUsdm::new(props).unwrap();
    ex.connect().await;
    let markets = ex.load_markets().await.unwrap();
    let mut target_markets: Vec<Market> = vec!();
    for m in markets {
        match m {
            Market{ref base, ref quote, ..} if base == "BTC" && quote == "USDT" => {
                target_markets.push((*m).clone());
            }
            _ => (),
        }
    }

    let mut stream = ex.watch_order_book(target_markets).await.unwrap();
    while let Some(x) = stream.next().await {
        print!("");
    }
}