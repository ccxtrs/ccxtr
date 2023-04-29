use ccxtr::exchange::BinanceUsdm;
use ccxtr::prelude::*;
#[tokio::main]
async fn main() {
    let ex = BinanceUsdm::new();
    ex.load_markets().await.unwrap();
}