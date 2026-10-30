use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;
use super::helpers::{rpc_ok, rpc_no_content};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/qrcodelink", get(qrcodelink))
        .route("/v1/qrcodelink/raw", get(qrcodelink_raw))
        .route("/v1/devices/{number}", post(link_device).get(list_devices))
        .route(
            "/v1/devices/{number}/{device_id}",
            delete(remove_device).put(update_device),
        )
        .route(
            "/v1/devices/{number}/local-data",
            delete(delete_local_data),
        )
        .route("/v1/devices/{number}/add", post(add_device))
}

#[derive(Deserialize)]
struct QrLinkQuery {
    #[serde(default)]
    device_name: Option<String>,
}

/// Remember the device name asked for on the QR/link request: signal-cli's startLink takes no parameters (the
/// name is given later, to finishLink), so we keep it here, keyed by the link URI, until POST /v1/devices/{number}.
fn remember_device_name(st: &AppState, result: &serde_json::Value, name: Option<String>) {
    let uri = result
        .as_str()
        .or_else(|| result.get("deviceLinkUri").and_then(|v| v.as_str()));
    if let (Some(uri), Some(name)) = (uri, name) {
        st.link_names.insert(uri.to_string(), name);
    }
}

async fn qrcodelink(
    axum::extract::Query(query): axum::extract::Query<QrLinkQuery>,
    State(st): State<AppState>,
) -> Response {
    match st.rpc("startLink", json!({})).await {
        Ok(result) => {
            remember_device_name(&st, &result, query.device_name);
            Json(result).into_response()
        }
        Err(e) => {
            let status = crate::state::rpc_error_status(&e);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

async fn qrcodelink_raw(
    axum::extract::Query(query): axum::extract::Query<QrLinkQuery>,
    State(st): State<AppState>,
) -> Response {
    // Special case: returns plain text, not JSON
    match st.rpc("startLink", json!({})).await {
        Ok(result) => {
            remember_device_name(&st, &result, query.device_name);
            let uri = result
                .as_str()
                .or_else(|| result.get("deviceLinkUri").and_then(|v| v.as_str()))
                .unwrap_or("");
            (StatusCode::OK, uri.to_string()).into_response()
        }
        Err(e) => {
            let status = crate::state::rpc_error_status(&e);
            (status, Json(json!({ "error": e }))).into_response()
        }
    }
}

#[derive(Deserialize)]
struct LinkDeviceBody {
    uri: String,
    #[serde(default)]
    device_name: Option<String>,
}

/// POST /v1/devices/{number} — complete a link started by /v1/qrcodelink.
///
/// signal-cli's `finishLink` takes `deviceLinkUri` and `deviceName` only; it
/// has no `account` (the account does not exist until the link completes -
/// the new account's number comes back in the result) and rejects unknown
/// fields. The `{number}` path segment is kept for API compatibility but is
/// not forwarded.
async fn link_device(
    Path(_number): Path<String>,
    State(st): State<AppState>,
    Json(body): Json<LinkDeviceBody>,
) -> Response {
    let mut params = json!({ "deviceLinkUri": body.uri });
    // The name given here wins; else the one asked for on /v1/qrcodelink for this same link URI.
    let name = body
        .device_name
        .or_else(|| st.link_names.remove(&body.uri).map(|(_, n)| n));
    if let Some(name) = name {
        params["deviceName"] = json!(name);
    }
    rpc_no_content(&st, "finishLink", params).await
}

async fn list_devices(Path(number): Path<String>, State(st): State<AppState>) -> Response {
    rpc_ok(&st, "listDevices", json!({ "account": number })).await
}

async fn remove_device(
    Path((number, device_id)): Path<(String, i64)>,
    State(st): State<AppState>,
) -> Response {
    rpc_no_content(&st, "removeDevice", json!({ "account": number, "deviceId": device_id })).await
}

async fn delete_local_data(Path(number): Path<String>, State(st): State<AppState>) -> Response {
    rpc_no_content(&st, "deleteLocalAccountData", json!({ "account": number })).await
}

#[derive(Deserialize)]
struct UpdateDeviceBody {
    device_name: String,
}

/// PUT /v1/devices/{number}/{device_id} — rename a linked device.
async fn update_device(
    Path((number, device_id)): Path<(String, i64)>,
    State(st): State<AppState>,
    Json(body): Json<UpdateDeviceBody>,
) -> Response {
    rpc_no_content(&st, "updateDevice", json!({
        "account": number,
        "deviceId": device_id,
        "deviceName": body.device_name,
    })).await
}

#[derive(Deserialize)]
struct AddDeviceBody {
    uri: String,
}

/// POST /v1/devices/{number}/add — approve linking a new device to this
/// account, from the primary device's side (the inverse of the
/// `/v1/devices/{number}` link flow, which is for this API instance
/// linking itself *to* another primary). Only works if this account is the
/// primary device; signal-cli's `addDevice` RPC method.
async fn add_device(
    Path(number): Path<String>,
    State(st): State<AppState>,
    Json(body): Json<AddDeviceBody>,
) -> Response {
    rpc_no_content(&st, "addDevice", json!({ "account": number, "uri": body.uri })).await
}
