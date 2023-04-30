use ccxtr::BinanceUsdm;
use ccxtr::Properties;
use ccxtr::Exchange;
#[tokio::main]
async fn main() {
    let props = Properties::builder().build();
    let ex = BinanceUsdm::new(props);
    let markets = ex.load_markets().await.unwrap();
    println!("{:?}", markets);
}