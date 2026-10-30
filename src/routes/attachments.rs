use axum::{
    Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{delete, get},
};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;
use super::helpers::{rpc_ok, rpc_no_content};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/attachments", get(list_attachments))
        .route("/v1/attachments/{attachment}", get(get_attachment))
        .route("/v1/attachments/{attachment}", delete(delete_attachment))
}

// KNOWN BROKEN, deliberately left as-is (not fixed as part of this pass):
// signal-cli has no `listAttachments` or `deleteAttachment` JSON-RPC
// methods at all - confirmed absent from signal-cli 0.14.7's full command
// surface (`Commands.java`) and from a full string-constant scan of the
// installed jar. There is no persistent, listable/deletable "attachment
// store" concept in signal-cli's RPC API to map these onto; attachments are
// only addressable per-message via `getAttachment`. Fixing this needs a
// product decision (e.g. maintaining our own attachment index keyed off
// received-message envelopes) rather than a method-name correction, so it's
// out of scope here. GET below (`getAttachment`) is the one real method in
// this file; it needs only `id` plus `account` (the `?account=` query
// param) - verified against a live 0.14.9 daemon, no recipient/group-id.

/// GET /v1/attachments — BROKEN: `listAttachments` is not a real signal-cli
/// RPC method. See note above.
async fn list_attachments(State(st): State<AppState>) -> Response {
    rpc_ok(&st, "listAttachments", json!({})).await
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

/// DELETE /v1/attachments/{attachment} — BROKEN: `deleteAttachment` is not
/// a real signal-cli RPC method. See note above.
async fn delete_attachment(
    State(st): State<AppState>,
    Path(attachment): Path<String>,
) -> Response {
    rpc_no_content(&st, "deleteAttachment", json!({ "id": attachment })).await
}
