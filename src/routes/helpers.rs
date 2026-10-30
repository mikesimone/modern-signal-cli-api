use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::state::{rpc_error_status, AppState};

/// Make an RPC call and return 200 OK with the JSON result on success.
pub async fn rpc_ok(st: &AppState, method: &str, params: Value) -> Response {
    let start = std::time::Instant::now();
    match st.rpc(method, params).await {
        Ok(result) => {
            tracing::info!(rpc_method = method, status = 200, latency_ms = start.elapsed().as_millis() as u64);
            Json(result).into_response()
        }
        Err(e) => {
            let status = rpc_error_status(&e);
            tracing::warn!(rpc_method = method, status = status.as_u16(), error = %e, latency_ms = start.elapsed().as_millis() as u64);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

/// Make an RPC call and return 201 Created with the JSON result on success.
pub async fn rpc_created(st: &AppState, method: &str, params: Value) -> Response {
    let start = std::time::Instant::now();
    match st.rpc(method, params).await {
        Ok(result) => {
            tracing::info!(rpc_method = method, status = 201, latency_ms = start.elapsed().as_millis() as u64);
            (StatusCode::CREATED, Json(result)).into_response()
        }
        Err(e) => {
            let status = rpc_error_status(&e);
            tracing::warn!(rpc_method = method, status = status.as_u16(), error = %e, latency_ms = start.elapsed().as_millis() as u64);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

/// Make an RPC call and return 204 No Content on success.
pub async fn rpc_no_content(st: &AppState, method: &str, params: Value) -> Response {
    let start = std::time::Instant::now();
    match st.rpc(method, params).await {
        Ok(_) => {
            tracing::info!(rpc_method = method, status = 204, latency_ms = start.elapsed().as_millis() as u64);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            let status = rpc_error_status(&e);
            tracing::warn!(rpc_method = method, status = status.as_u16(), error = %e, latency_ms = start.elapsed().as_millis() as u64);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

/// Renames legacy (bbernhard signal-cli-rest-api style) request-body fields to the names signal-cli actually
/// reads. signal-cli silently ignores parameter names it does not know, so a body such as `{"reaction": "👍"}`
/// used to be forwarded untouched and did nothing. Real names always win: if the caller sent both, the legacy
/// field is dropped.
pub fn translate_legacy_params(params: &mut Value, renames: &[(&str, &str)]) {
    if let Some(obj) = params.as_object_mut() {
        for (legacy, real) in renames {
            if let Some(v) = obj.remove(*legacy) {
                obj.entry(real.to_string()).or_insert(v);
            }
        }
    }
}

/// JSON-RPC error code inside an RPC error string, if it is one.
pub fn rpc_error_code(err: &str) -> Option<i64> {
    serde_json::from_str::<Value>(err).ok()?.get("code")?.as_i64()
}

/// signal-cli answers an unknown method with -32601 "Method not implemented".
pub fn is_method_not_found(err: &str) -> bool {
    rpc_error_code(err) == Some(-32601)
}

/// `{"error": msg}` with the given status.
pub fn json_error(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({ "error": msg.into() }))).into_response()
}

/// Make an RPC call; None means signal-cli does not have that method (so the caller should use its own
/// implementation), anything else is the call's real outcome.
pub async fn rpc_if_supported(st: &AppState, method: &str, params: Value) -> Option<Result<Value, String>> {
    match st.rpc(method, params).await {
        Err(e) if is_method_not_found(&e) => {
            tracing::info!(rpc_method = method, "signal-cli lacks this method, using the built-in implementation");
            None
        }
        other => Some(other),
    }
}

/// Turn an RPC outcome into a response: `ok_status` + JSON result, or the usual error mapping.
pub fn rpc_outcome(method: &str, result: Result<Value, String>, ok_status: StatusCode) -> Response {
    match result {
        Ok(v) if ok_status == StatusCode::NO_CONTENT => {
            let _ = v;
            ok_status.into_response()
        }
        Ok(v) => (ok_status, Json(v)).into_response(),
        Err(e) => {
            let status = rpc_error_status(&e);
            tracing::warn!(rpc_method = method, status = status.as_u16(), error = %e);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}
