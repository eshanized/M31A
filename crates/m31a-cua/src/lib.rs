//! Accessibility-First Computer Use Agent (CUA) for M31A.
//!
//! Provides native application inspection, semantic accessibility tree traversal,
//! safe background observation, and element-oriented UI automation.

pub mod backend;
pub mod error;
pub mod model;
pub mod session;

pub use backend::{CuaBackend, SimulatedBackend};
pub use error::CuaError;
pub use model::{
    AccessibleElement, ActionResult, AppInfo, AppState, ElementAction, ElementRole, MouseAction,
    Rect, WindowInfo,
};
pub use session::CuaSessionManager;
