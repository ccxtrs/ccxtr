pub(in super) fn to_unified_symbol(exchange_symbol: &str) -> String {
    exchange_symbol.to_uppercase()
}

pub(in super) fn is_active(status: Option<String>) -> bool {
    status.map(|status| status == "TRADING").unwrap_or(false)
}

