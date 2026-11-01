//! `m31a-diff`: Unified and intra-line diff engine, theme-aware rendering, and scrollable diff viewer.

pub mod churn;
pub mod engine;
pub mod render;
pub mod turn_tracker;
pub mod view;

pub use churn::{ChurnVerdict, DiffChurnStats, DiffMinimizer};
pub use engine::{DiffEngine, DiffLine, DiffLineKind, DiffStats, WordSpan};
pub use render::DiffRenderer;
pub use turn_tracker::{FileMutationKind, TrackedFileEntry, TurnDiffTracker};
pub use view::DiffViewerState;
