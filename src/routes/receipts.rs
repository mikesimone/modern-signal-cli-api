use axum::{
    Router,
    extract::{Path, State},
    response::Response,
    routing::post,
    Json,
};
use serde_json::{json, Value};

use crate::state::AppState;
use super::helpers::{rpc_ok, translate_legacy_params};

/// Legacy body field -> signal-cli's real sendReceipt parameter.
const LEGACY: &[(&str, &str)] = &[("receipt_type", "type"), ("timestamp", "targetTimestamp")];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/receipts/{number}", post(send_receipt))
}

/// POST /v1/receipts/{number} — send a read/delivery receipt.
async fn send_receipt(
    State(st): State<AppState>,
    Path(number): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut params = body;
    translate_legacy_params(&mut params, LEGACY);
    params["account"] = json!(number);
    rpc_ok(&st, "sendReceipt", params).await
}
