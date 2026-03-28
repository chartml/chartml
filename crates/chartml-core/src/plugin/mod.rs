pub mod data_source;
pub mod transform;
pub mod renderer;
pub mod resolver;

pub use data_source::{DataSource, DataSpec, FetchOptions};
pub use transform::{TransformMiddleware, TransformContext, TransformResult};
pub use renderer::{ChartRenderer, ChartConfig};
pub use resolver::{DatasourceResolver, ConnectionConfig};


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_data_spec_creation() {
        let spec = DataSpec {
            provider: "inline".to_string(),
            rows: Some(vec![serde_json::json!({"x": 1})]),
            url: None,
            endpoint: None,
        };
        assert_eq!(spec.provider, "inline");
        assert!(spec.rows.is_some());
        assert_eq!(spec.rows.unwrap().len(), 1);
        assert!(spec.url.is_none());
        assert!(spec.endpoint.is_none());
    }

    #[test]
    fn test_chart_config_creation() {
        use crate::spec::VisualizeSpec;

        let visualize: VisualizeSpec = serde_yaml::from_str(
            r#"
            type: bar
            columns: month
            rows: revenue
            "#,
        )
        .unwrap();

        let config = ChartConfig {
            visualize,
            title: Some("Test Chart".to_string()),
            width: 800.0,
            height: 600.0,
            colors: vec!["#ff0000".to_string(), "#00ff00".to_string()],
        };
        assert_eq!(config.title, Some("Test Chart".to_string()));
        assert_eq!(config.width, 800.0);
        assert_eq!(config.height, 600.0);
        assert_eq!(config.colors.len(), 2);
    }

    #[test]
    fn test_transform_result_creation() {
        use crate::data::DataTable;

        let mut metadata = HashMap::new();
        metadata.insert("row_count".to_string(), serde_json::json!(5));

        let result = TransformResult {
            data: DataTable::from_rows(&[]).unwrap(),
            metadata,
        };
        assert!(result.data.is_empty());
        assert_eq!(result.metadata.len(), 1);
        assert_eq!(result.metadata["row_count"], serde_json::json!(5));
    }

    #[test]
    fn test_fetch_options_default() {
        let options = FetchOptions::default();
        assert!(options.cache_ttl.is_none());
    }
}
