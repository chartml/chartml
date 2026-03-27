use serde::{Deserialize, Serialize};

use super::chart::ChartStyleSpec;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSpec {
    pub version: u32,
    pub style: StyleRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StyleRef {
    Named(String),
    Inline(Box<ChartStyleSpec>),
}
