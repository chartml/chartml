pub mod stack;
pub mod axes;
pub mod labels;
pub mod margins;
pub mod legend;

pub use stack::{StackLayout, StackOffset, StackOrder, StackedPoint};
pub use axes::{AxisLayout, AxisPosition, TickMark, CategoryTickMark, adaptive_tick_count};
pub use labels::{LabelStrategy, LabelStrategyConfig, approximate_text_width, truncate_label, strategic_indices, compute_skip_factor};
pub use margins::{Margins, MarginConfig, calculate_margins};
pub use legend::{LegendItem, LegendConfig, LegendAlignment, LegendLayoutResult, calculate_legend_layout};
