use derive_builder::Builder;

use crate::model::MarginMode;

#[derive(Default, Builder, Debug)]
#[builder(default)]
pub struct FetchBalanceParams {
    pub margin_mode: Option<MarginMode>,
}

