use axum::{
    Router,
    extract::{Path, State, WebSocketUpgrade, ws},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json,
};
use base64::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::AppState;
use super::helpers::{rpc_ok, rpc_created, translate_legacy_params};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/send", post(send_v1))
        .route("/v2/send", post(send_v2))
        .route("/v1/receive/{number}", get(receive_ws))
        .route("/v1/remote-delete/{number}", delete(remote_delete))
}

const ATTACHMENT_SPILL_DIR: &str = "outgoing-attachments";
static ATTACHMENT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// signal-cli parses incoming JSON-RPC requests with Jackson, which enforces
/// a hard 20,000,000-character limit on any single JSON string value
/// (`StreamReadConstraints.getMaxStringLength()`). Attachments arrive here as
/// base64 data URIs, so any file whose base64 form crosses that line (raw
/// size a bit over ~15MB) trips a `StreamConstraintsException` that
/// signal-cli doesn't handle cleanly - it kills the one shared JSON-RPC
/// connection this whole process depends on, not just the offending
/// request. signal-cli's `attachment` field also accepts a plain local file
/// path, so spill data-URI attachments to disk and hand signal-cli the path
/// instead, sidestepping the JVM's JSON parser for the bulky part entirely.
/// Files are cleaned up once the send RPC call returns (success or error).
pub(crate) struct SpilledAttachments(Vec<PathBuf>);

impl Drop for SpilledAttachments {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) fn spill_attachments_to_disk(body: &mut Value) -> std::io::Result<SpilledAttachments> {
    let mut written = Vec::new();
    if let Some(Value::Array(attachments)) = body.get_mut("attachment") {
        let dir = std::path::Path::new(ATTACHMENT_SPILL_DIR);
        std::fs::create_dir_all(dir)?;
        for entry in attachments.iter_mut() {
            let Value::String(s) = entry else { continue };
            let Some(rest) = s.strip_prefix("data:") else { continue };
            let Some((meta, b64_data)) = rest.split_once(";base64,") else { continue };
            let filename = meta
                .split(';')
                .find_map(|part| part.strip_prefix("filename="))
                .unwrap_or("attachment");
            let ext = std::path::Path::new(filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("bin");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(b64_data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            let unique = ATTACHMENT_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = dir.join(format!("{}-{unique}.{ext}", std::process::id()));
            std::fs::write(&path, &bytes)?;
            *entry = Value::String(path.to_string_lossy().into_owned());
            written.push(path);
        }
    }
    Ok(SpilledAttachments(written))
}

/// POST /v1/send — send a message (v1, simple).
async fn send_v1(
    State(st): State<AppState>,
    Json(mut body): Json<Value>,
) -> Response {
    let _spilled = match spill_attachments_to_disk(&mut body) {
        Ok(guard) => guard,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    };
    rpc_created(&st, "send", body).await
}

/// POST /v2/send — send a message (v2, extended). Increments sent counter.
async fn send_v2(
    State(st): State<AppState>,
    Json(mut body): Json<Value>,
) -> Response {
    let start = std::time::Instant::now();
    let _spilled = match spill_attachments_to_disk(&mut body) {
        Ok(guard) => guard,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    };
    match st.rpc("send", body).await {
        Ok(result) => {
            st.metrics.inc_sent();
            tracing::info!(rpc_method = "send", status = 201, latency_ms = start.elapsed().as_millis() as u64);
            (axum::http::StatusCode::CREATED, Json(result)).into_response()
        }
        Err(e) => {
            let status = crate::state::rpc_error_status(&e);
            tracing::warn!(rpc_method = "send", status = status.as_u16(), error = %e, latency_ms = start.elapsed().as_millis() as u64);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

/// GET /v1/receive/{number} — WebSocket endpoint for real-time messages.
async fn receive_ws(
    State(st): State<AppState>,
    Path(_number): Path<String>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_ws(socket, st))
}

async fn handle_ws(mut socket: ws::WebSocket, st: AppState) {
    st.metrics.ws_clients.fetch_add(1, Ordering::Relaxed);
    let mut rx = st.broadcast_tx.subscribe();

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(ws::Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(ws::Message::Close(_))) | None => break,
                    _ => {} // ignore client-sent frames
                }
            }
        }
    }

    st.metrics.ws_clients.fetch_sub(1, Ordering::Relaxed);
}

/// DELETE /v1/remote-delete/{number} — remotely delete a sent message.
async fn remote_delete(
    State(st): State<AppState>,
    Path(number): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut params = body;
    translate_legacy_params(&mut params, &[("timestamp", "targetTimestamp")]);
    params["account"] = json!(number);
    rpc_ok(&st, "remoteDelete", params).await
}
