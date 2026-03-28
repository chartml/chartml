use serde::{Deserialize, Serialize};

use super::chart::LayoutSpec;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsSpec {
    pub version: u32,
    pub name: Option<String>,
    pub params: Vec<ParamDef>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub id: String,
    #[serde(rename = "type")]
    pub param_type: String,
    pub label: String,
    pub options: Option<Vec<String>>,
    pub default: Option<serde_json::Value>,
    pub placeholder: Option<String>,
    pub layout: Option<LayoutSpec>,
}
