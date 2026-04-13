pub mod stack;
pub mod axes;
pub mod labels;
pub mod margins;
pub mod legend;

pub use stack::{StackLayout, StackOffset, StackOrder, StackedPoint};
pub use axes::{AxisLayout, AxisPosition, TickMark, CategoryTickMark, adaptive_tick_count};
pub use labels::{LabelStrategy, LabelStrategyConfig, TextMetrics, approximate_text_width, measure_text, truncate_label, truncate_label_with_metrics, strategic_indices, compute_skip_factor};
pub use margins::{Margins, MarginConfig, calculate_margins};
pub use legend::{LegendItem, LegendConfig, LegendAlignment, LegendLayoutResult, LegendMark, calculate_legend_layout, generate_legend_elements};
