use axum::{
    http::StatusCode,
    response::IntoResponse,
    Json,
    Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{delete, get},
};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;
use super::helpers::{json_error, rpc_if_supported, rpc_ok, rpc_outcome};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/attachments", get(list_attachments))
        .route("/v1/attachments/{attachment}", get(get_attachment))
        .route("/v1/attachments/{attachment}", delete(delete_attachment))
}

// signal-cli has no `listAttachments` / `deleteAttachment` JSON-RPC methods (checked against 0.14.9), so these two
// try the real method first and, only if signal-cli says it does not implement it, work on signal-cli's attachment
// directory (<data dir>/attachments) directly - see native.rs. Attachments are shared by all accounts, so these
// are not per-account.

/// GET /v1/attachments - list locally stored attachments: `[{"id", "size", "modified" (ms)}]`.
async fn list_attachments(State(st): State<AppState>) -> Response {
    if let Some(res) = rpc_if_supported(&st, "listAttachments", json!({})).await {
        return rpc_outcome("listAttachments", res, StatusCode::OK);
    }
    match crate::native::list_attachments(&st.data_dir) {
        Ok(list) => Json(list).into_response(),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("cannot read the attachment directory: {e}")),
    }
}

#[derive(Deserialize)]
struct GetAttachmentQuery {
    /// Account (phone number) the attachment belongs to. signal-cli's daemon
    /// needs this to pick a manager when more than one account is registered;
    /// without it `getAttachment` fails with "Method requires valid account
    /// parameter". Optional so single-account callers keep working unchanged.
    #[serde(default)]
    account: Option<String>,
}

/// GET /v1/attachments/{attachment}[?account=+1555...] — retrieve a specific attachment.
async fn get_attachment(
    State(st): State<AppState>,
    Path(attachment): Path<String>,
    Query(query): Query<GetAttachmentQuery>,
) -> Response {
    let mut params = json!({ "id": attachment });
    if let Some(account) = query.account {
        params["account"] = json!(account);
    }
    rpc_ok(&st, "getAttachment", params).await
}

/// DELETE /v1/attachments/{attachment} - delete a locally stored attachment (204, or 404 if there is none).
async fn delete_attachment(
    State(st): State<AppState>,
    Path(attachment): Path<String>,
) -> Response {
    if let Some(res) = rpc_if_supported(&st, "deleteAttachment", json!({ "id": attachment })).await {
        return rpc_outcome("deleteAttachment", res, StatusCode::NO_CONTENT);
    }
    match crate::native::delete_attachment(&st.data_dir, &attachment) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => json_error(StatusCode::NOT_FOUND, format!("no attachment with id {attachment}")),
        Err(e) => json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("cannot delete {attachment}: {e}")),
    }
}
