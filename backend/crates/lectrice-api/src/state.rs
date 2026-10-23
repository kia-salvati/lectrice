use lectrice_app::services::Services;

/// Shared handles for controllers, built once in the composition root (`composition.rs`).
#[derive(Clone)]
pub struct AppState {
    /// Speech and exports are `None` when the speech engine failed to start; the server still runs and reports it
    /// in `/api/health`.
    pub services: Services,
    /// `upload.max_mb` in bytes; the upload route stops reading past it.
    pub upload_max_bytes: usize,
}
