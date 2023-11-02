use std::collections::HashMap;
use crate::model::Market;

#[derive(Clone)]
pub struct Unifier {
    unified_market_to_symbol_id: HashMap<Market, String>,
    symbol_id_to_unified_market: HashMap<String, Market>,

}

impl Unifier {
    pub fn new() -> Self {
        Self {
            unified_market_to_symbol_id: HashMap::new(),
            symbol_id_to_unified_market: HashMap::new(),
        }
    }

    pub fn insert_market_symbol_id(&mut self, market: &Market, symbol_id: &String) {
        self.unified_market_to_symbol_id.insert(market.clone(), symbol_id.clone());
        self.symbol_id_to_unified_market.insert(symbol_id.clone(), market.clone());
    }

    pub fn get_symbol_id(&self, market: &Market) -> Option<String> {
        self.unified_market_to_symbol_id.get(market).cloned()
    }

    pub fn get_market(&self, symbol_id: &String) -> Option<Market> {
        self.symbol_id_to_unified_market.get(symbol_id).cloned()
    }

    pub fn reset(&mut self) {
        self.unified_market_to_symbol_id.clear();
        self.symbol_id_to_unified_market.clear();
    }
}