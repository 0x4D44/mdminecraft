//! Billboard Rendering Pipeline

use anyhow::Result;

/// Billboard rendering pipeline (experimental)
///
/// Currently a stub; gated by the `ui3d_billboards` Cargo feature to avoid
/// accidental use in production builds until fully implemented.
pub struct BillboardPipeline {
    // TODO: Implement billboard pipeline
}

impl BillboardPipeline {
    /// Create a new billboard pipeline
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}
