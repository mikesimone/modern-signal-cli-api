use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Map, Value};

use crate::native::{camel, load_settings, save_settings, ACCOUNT_SETTINGS, CONFIGURATION_SETTINGS};
use crate::state::{rpc_error_status, AppState};
use super::helpers::{json_error, rpc_if_supported, rpc_outcome};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/v1/configuration",
            get(get_global_config).post(set_global_config),
        )
        .route(
            "/v1/configuration/{number}/settings",
            get(get_account_config).post(set_account_config),
        )
}

// signal-cli has no getConfiguration/setConfiguration/getAccountSettings/setAccountSettings JSON-RPC methods (those
// exist only on its D-Bus interface, or not at all). What it does have is write-only: `updateConfiguration`
// (read receipts, typing indicators, link previews, sealed-sender indicators - synced to linked devices) and
// `updateAccount` (device name, number sharing, username, ...). So:
//   * GET/POST /v1/configuration/{number}/settings  -> POST applies the settings through those two real methods and
//     remembers what was set (signal-cli cannot report it back); GET returns what was last set through this API.
//   * GET /v1/configuration -> the daemon's fixed trust mode and signal-cli version. The trust mode is a daemon
//     start-up flag (`--trust-new-identities`), not changeable at runtime, so POST only accepts the value already
//     in force.
// Each route tries signal-cli's own getter/setter first (rpc_if_supported) so a future signal-cli that adds them wins.

/// Trust mode of the daemon this API spawns (signal-cli's default; the daemon is started without the flag).
const TRUST_MODE: &str = "on-first-use";

async fn get_global_config(State(st): State<AppState>) -> Response {
    if let Some(res) = rpc_if_supported(&st, "getConfiguration", json!({})).await {
        return rpc_outcome("getConfiguration", res, StatusCode::OK);
    }
    let mut out = json!({ "trustMode": TRUST_MODE });
    if let Ok(v) = st.rpc("version", json!({})).await {
        if let Some(ver) = v.get("version") {
            out["signalCli"] = json!({ "version": ver });
        }
    }
    Json(out).into_response()
}

async fn set_global_config(State(st): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(res) = rpc_if_supported(&st, "setConfiguration", body.clone()).await {
        return rpc_outcome("setConfiguration", res, StatusCode::NO_CONTENT);
    }
    let trust = body.get("trustMode").or_else(|| body.get("trust_mode")).and_then(|v| v.as_str());
    let only_trust = body.as_object().map(|o| o.keys().all(|k| k == "trustMode" || k == "trust_mode")).unwrap_or(false);
    if only_trust && trust == Some(TRUST_MODE) {
        return StatusCode::NO_CONTENT.into_response(); // already in force
    }
    json_error(
        StatusCode::NOT_IMPLEMENTED,
        format!("the daemon's trust mode is fixed at start-up ({TRUST_MODE}); signal-cli cannot change it at runtime"),
    )
}

async fn get_account_config(Path(number): Path<String>, State(st): State<AppState>) -> Response {
    if let Some(res) = rpc_if_supported(&st, "getAccountSettings", json!({ "account": number })).await {
        return rpc_outcome("getAccountSettings", res, StatusCode::OK);
    }
    Json(load_settings(&st.data_dir, &number)).into_response()
}

async fn set_account_config(
    Path(number): Path<String>,
    State(st): State<AppState>,
    Json(body): Json<Value>,
) -> Response {
    // Real signal-cli setter, if a newer one ever has it.
    let mut raw = body.clone();
    raw["account"] = json!(number);
    if let Some(res) = rpc_if_supported(&st, "setAccountSettings", raw).await {
        return rpc_outcome("setAccountSettings", res, StatusCode::NO_CONTENT);
    }

    let Some(obj) = body.as_object() else {
        return json_error(StatusCode::BAD_REQUEST, "expected a JSON object of settings");
    };
    let mut config = Map::new();
    let mut account = Map::new();
    let mut unknown = Vec::new();
    for (k, v) in obj {
        let key = camel(k);
        if CONFIGURATION_SETTINGS.contains(&key.as_str()) {
            config.insert(key, v.clone());
        } else if ACCOUNT_SETTINGS.contains(&key.as_str()) {
            account.insert(key, v.clone());
        } else {
            unknown.push(k.clone());
        }
    }
    if !unknown.is_empty() {
        return json_error(
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported setting(s): {}. Supported: {}",
                unknown.join(", "),
                CONFIGURATION_SETTINGS.iter().chain(ACCOUNT_SETTINGS.iter()).copied().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    if config.is_empty() && account.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "no settings given");
    }

    let mut applied = Map::new();
    for (method, params) in [("updateConfiguration", config), ("updateAccount", account)] {
        if params.is_empty() {
            continue;
        }
        let mut call = Value::Object(params.clone());
        call["account"] = json!(number);
        if let Err(e) = st.rpc(method, call).await {
            tracing::warn!(rpc_method = method, error = %e, "settings update failed");
            if !applied.is_empty() {
                let _ = save_settings(&st.data_dir, &number, &applied); // keep what did go through
            }
            return (rpc_error_status(&e), Json(json!({ "error": e }))).into_response();
        }
        applied.extend(params);
    }
    if let Err(e) = save_settings(&st.data_dir, &number, &applied) {
        tracing::warn!(error = %e, "settings applied but could not be remembered");
    }
    StatusCode::NO_CONTENT.into_response()
}
