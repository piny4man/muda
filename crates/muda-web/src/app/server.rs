use leptos::prelude::*;

#[server]
pub async fn health() -> Result<String, ServerFnError> {
    Ok("ok".into())
}

/// Stub for a later optional upload path for formats the browser cannot strip.
/// v1 does not accept image bytes.
#[server]
pub async fn upload_unsupported(_filename: String) -> Result<(), ServerFnError> {
    Err(ServerFnError::ServerError(
        "501 Not Implemented: optional upload for unsupported formats is not available in v1"
            .into(),
    ))
}
