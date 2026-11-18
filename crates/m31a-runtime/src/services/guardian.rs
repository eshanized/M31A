//! Guardian security reviewer and circuit breaker service.

use m31a_guardian::GuardianReviewer;
use std::sync::Arc;

/// Initialize the Guardian reviewer for the given active model.
pub fn init_guardian(model: &str) -> Arc<GuardianReviewer> {
    Arc::new(GuardianReviewer::new(model))
}
