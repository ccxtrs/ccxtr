use crate::prelude::Result;
use crate::exchange::Exchange;
use async_trait::async_trait;

pub struct BinanceUsdm {
}


impl BinanceUsdm {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Exchange for BinanceUsdm {
    async fn load_markets(&self) -> Result<()> {
        Ok(())
    }
}
