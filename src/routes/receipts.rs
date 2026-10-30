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
    // Receipts go to a person; signal-cli's sendReceipt has no group parameter. Drop one a caller sent
    // (bbernhard-style bodies include it) rather than forward a field signal-cli would silently ignore.
    if let Some(obj) = params.as_object_mut() {
        obj.remove("group-id");
        obj.remove("groupId");
        obj.remove("group_id");
    }
    params["account"] = json!(number);
    rpc_ok(&st, "sendReceipt", params).await
}
