pub mod config;
pub mod crypto;
pub mod db;
pub mod mdns;
pub mod scheduler;
pub mod sideload;
pub mod state;

pub async fn export_pairing_file(
    axum::extract::State(state): axum::extract::State<state::AppState>,
    axum::extract::Path(device_id): axum::extract::Path<String>,
) -> Result<impl axum::response::IntoResponse, (axum::http::StatusCode, &'static str)> {
    use axum::http::{header, StatusCode};

    let device = db::get_device(&state.db, &device_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load device"))?
        .ok_or((StatusCode::NOT_FOUND, "Device not found"))?;
    let bytes = state.crypto.decrypt(&device.pairing_blob).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to decrypt pairing",
        )
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"pairingFile.plist\"",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    ))
}
