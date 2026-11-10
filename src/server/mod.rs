pub mod companion;
pub mod config;
pub mod crypto;
pub mod db;
pub mod mdns;
pub mod scheduler;
pub mod sideload;
pub mod state;

#[derive(serde::Deserialize)]
pub struct CertificateDownload {
    p12_b64: String,
}

pub async fn download_certificate(
    axum::extract::Form(certificate): axum::extract::Form<CertificateDownload>,
) -> Result<impl axum::response::IntoResponse, (axum::http::StatusCode, &'static str)> {
    use axum::http::{header, StatusCode};
    use base64::prelude::*;

    let bytes = BASE64_STANDARD
        .decode(&certificate.p12_b64)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid certificate data"))?;
    if bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Certificate is empty"));
    }

    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"ALTCertificate.p\"",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        bytes,
    ))
}

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
