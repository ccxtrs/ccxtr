use crate::prelude::Result;
use crate::exchange::Exchange;
use crate::exchange::binance::Properties;

use async_trait::async_trait;


pub struct BinanceUsdm {
    host: String,
    port: u16,
    api_key: String,
    secret_key: String,

    client: reqwest::Client,
}


impl BinanceUsdm {
    pub fn new(props: Properties) -> Self {
        let client = reqwest::Client::new();

        Self {
            host: props.host,
            port: props.port,
            api_key: props.api_key,
            secret_key: props.secret_key,
            client,
        }
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&self) -> Result<()> {
        Ok(())
    }
}
