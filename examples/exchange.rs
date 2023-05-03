use ccxtr::{BinanceUsdm, PropertiesBuilder};
use ccxtr::Exchange;
use ccxtr::model::Market;

#[tokio::main]
async fn main() {
    let props = PropertiesBuilder::new().build();
    let ex = BinanceUsdm::new(props);
    let markets = ex.fetch_markets().await.unwrap();
    for m in markets {
        match m {
            Market{ref base, ref quote, ..} if base == "BTC" && quote == "USDT" => {
                println!("{:#?}", m);
            }
            _ => (),
        }
    }
}