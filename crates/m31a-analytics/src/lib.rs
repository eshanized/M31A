//! `m31a-analytics`: Interactive token accounting, latency metrics, and terminal dashboard.

pub mod charts;
pub mod dashboard;
pub mod metrics;

pub use charts::{render_bar, render_sparkline};
pub use dashboard::AnalyticsDashboard;
pub use metrics::SessionTokenMetrics;
