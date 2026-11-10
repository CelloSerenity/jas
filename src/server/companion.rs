use std::{io::Cursor, path::PathBuf};

use anyhow::Context;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use isideload::sideload::builder::MaxCertsBehavior;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{crypto::Crypto, db, sideload, state::AppState};

type ApiError = (StatusCode, &'static str);

#[derive(Clone)]
struct CompanionState {
    app: AppState,
    receipts: Crypto,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    id: String,
    device_id: String,
    account_id: String,
    bundle_id: String,
    display_name: String,
    version: Option<String>,
    ipa_path: PathBuf,
    expires_at: i64,
}

#[derive(Deserialize)]
struct Completion {
    receipt: String,
}

pub fn routes<S: Clone + Send + Sync + 'static>(app: AppState) -> Router<S> {
    let Ok(token) = std::env::var("JAS_COMPANION_TOKEN") else {
        return Router::new();
    };
    if token.len() < 32 {
        tracing::warn!("JAS_COMPANION_TOKEN is too short. Companion API disabled.");
        return Router::new();
    }
    let token_hash: [u8; 32] = Sha256::digest(format!("Bearer {token}")).into();

    Router::new()
        .route("/api/companion/install", get(accounts).post(prepare))
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .route(
            "/api/companion/complete",
            post(complete).layer(DefaultBodyLimit::max(4096)),
        )
        .route_layer(middleware::from_fn(
            move |request: Request, next: Next| async move {
                let authorized =
                    request
                        .headers()
                        .get(header::AUTHORIZATION)
                        .is_some_and(|value| {
                            let hash: [u8; 32] = Sha256::digest(value.as_bytes()).into();
                            constant_time_eq::constant_time_eq_32(&hash, &token_hash)
                        });
                if authorized {
                    next.run(request).await
                } else {
                    (StatusCode::UNAUTHORIZED, "Invalid JAS companion token").into_response()
                }
            },
        ))
        .with_state(CompanionState {
            app,
            receipts: Crypto::new(rand::random()),
        })
}

fn internal_error(error: impl std::fmt::Display) -> ApiError {
    tracing::warn!("Companion request failed: {error}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "JAS could not complete the request. Check the server logs.",
    )
}

async fn accounts(State(state): State<CompanionState>) -> Result<impl IntoResponse, ApiError> {
    let accounts = db::list_accounts(&state.app.db)
        .await
        .map_err(internal_error)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(accounts)))
}

async fn prepare(
    State(state): State<CompanionState>,
    headers: HeaderMap,
    ipa: Bytes,
) -> Result<Response, ApiError> {
    let get_header = |name, message| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .ok_or((StatusCode::BAD_REQUEST, message))
    };
    let udid = get_header("x-jas-device-udid", "USB device UDID is required")?;
    let account_id = get_header("x-jas-account-id", "Apple account is required")?;
    let device = db::list_devices(&state.app.db)
        .await
        .map_err(internal_error)?
        .into_iter()
        .find(|device| device.udid.eq_ignore_ascii_case(udid))
        .ok_or((
            StatusCode::NOT_FOUND,
            "Add this device to JAS before signing",
        ))?;
    let account = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT apple_id, session_blob FROM apple_accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_optional(&state.app.db)
    .await
    .map_err(internal_error)?
    .ok_or((StatusCode::NOT_FOUND, "Apple account not found in JAS"))?;
    let pairing = state
        .app
        .crypto
        .decrypt(&device.pairing_blob)
        .map_err(internal_error)?;

    let (temp, app_path, metadata) = tokio::task::spawn_blocking({
        let ipa = ipa.clone();
        move || -> anyhow::Result<_> {
            let temp = tempfile::tempdir()?;
            let mut archive = zip::ZipArchive::new(Cursor::new(&ipa))?;
            archive.extract_with_options(
                temp.path(),
                zip::read::ExtractOptions::default().with_size_limit(2 * 1024 * 1024 * 1024),
            )?;
            let metadata = sideload::read_ipa_info(&ipa)?;
            let mut apps = Vec::new();
            for entry in std::fs::read_dir(temp.path().join("Payload"))? {
                let entry = entry?;
                if entry.file_type()?.is_dir()
                    && entry.path().extension().is_some_and(|ext| ext == "app")
                {
                    apps.push(entry.path());
                }
            }
            let [app_path] = apps.try_into().ok().context("IPA must contain one app")?;
            Ok((temp, app_path, metadata))
        }
    })
    .await
    .map_err(internal_error)?
    .map_err(|error| {
        tracing::warn!("Companion could not unpack IPA: {error}");
        (StatusCode::UNPROCESSABLE_ENTITY, "Could not unpack the IPA")
    })?;

    let anisette = state.app.anisette_url.read().await.clone();
    let signer = sideload::build_signer(
        &state.app.db,
        &state.app.crypto,
        &account.0,
        &account.1,
        &anisette,
        MaxCertsBehavior::Error,
    )
    .await
    .map_err(internal_error)?;
    let signed_path = sideload::sign_ipa(signer, app_path, &device.name, &device.udid, &pairing)
        .await
        .map_err(internal_error)?;

    let dir = PathBuf::from(&state.app.config.storage.ipa_dir);
    let (bundle_id, signed, ipa_path) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let _temp = temp;
            let bundle_id = sideload::read_bundle_id_from_app(&signed_path)?;
            let signed = sideload::create_ipa(&signed_path)?;
            let hash = hex::encode(Sha256::digest(&ipa));
            let path = dir.join(format!("companion-{hash}.ipa"));
            std::fs::create_dir_all(&dir)?;
            let mut file = tempfile::NamedTempFile::new_in(dir)?;
            std::io::Write::write_all(&mut file, &ipa)?;
            file.persist(&path)?;
            Ok((bundle_id, signed, path))
        })
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;

    let receipt = Receipt {
        id: Uuid::new_v4().to_string(),
        device_id: device.id,
        account_id: account_id.to_string(),
        bundle_id,
        display_name: metadata.display_name.chars().take(128).collect(),
        version: metadata
            .version
            .map(|version| version.chars().take(64).collect()),
        ipa_path,
        expires_at: chrono::Utc::now().timestamp() + 24 * 60 * 60,
    };
    let payload = serde_json::to_vec(&receipt).map_err(internal_error)?;
    let ticket = URL_SAFE_NO_PAD.encode(state.receipts.encrypt(&payload));

    Ok((
        [
            (header::CONTENT_TYPE.as_str(), "application/octet-stream"),
            (header::CACHE_CONTROL.as_str(), "no-store"),
            ("x-jas-install-receipt", &ticket),
        ],
        signed,
    )
        .into_response())
}

async fn complete(
    State(state): State<CompanionState>,
    Json(completion): Json<Completion>,
) -> Result<StatusCode, ApiError> {
    let invalid_receipt = (
        StatusCode::BAD_REQUEST,
        "Invalid or expired JAS install receipt",
    );
    let encrypted = URL_SAFE_NO_PAD
        .decode(completion.receipt)
        .map_err(|_| invalid_receipt)?;
    let payload = state
        .receipts
        .decrypt(&encrypted)
        .map_err(|_| invalid_receipt)?;
    let receipt: Receipt = serde_json::from_slice(&payload).map_err(|_| invalid_receipt)?;

    let now = chrono::Utc::now().timestamp();
    let mut tx = state
        .app
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(internal_error)?;
    let recorded: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jobs WHERE id = ?)")
        .bind(&receipt.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(internal_error)?;
    if recorded {
        return Ok(StatusCode::NO_CONTENT);
    }
    if now > receipt.expires_at {
        return Err(invalid_receipt);
    }

    let app_id: String = sqlx::query_scalar(
        "INSERT INTO apps
         (id, device_id, bundle_id, display_name, version, ipa_path, account_id, installed_at, last_refreshed)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(device_id, bundle_id) DO UPDATE SET
           display_name = excluded.display_name, version = COALESCE(excluded.version, apps.version),
           ipa_path = excluded.ipa_path, account_id = excluded.account_id,
           installed_at = COALESCE(apps.installed_at, excluded.installed_at),
           last_refreshed = excluded.last_refreshed
         RETURNING id",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(receipt.device_id)
    .bind(receipt.bundle_id)
    .bind(receipt.display_name)
    .bind(receipt.version)
    .bind(receipt.ipa_path.to_string_lossy().as_ref())
    .bind(receipt.account_id)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    sqlx::query(
        "INSERT INTO jobs (id, app_id, kind, status, started_at, finished_at, progress, stage)
         VALUES (?, ?, 'install', 'done', ?, ?, 100, 'USB')",
    )
    .bind(receipt.id)
    .bind(app_id)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;
    tx.commit().await.map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}
