use axum::{
    Router,
    extract::{Path, State},
    response::Response,
    routing::{delete, post},
    Json,
};
use serde_json::{json, Value};

use crate::state::AppState;
use super::helpers::{rpc_created, rpc_no_content, translate_legacy_params};

/// Legacy body field -> signal-cli's real sendReaction parameter.
const LEGACY: &[(&str, &str)] = &[("reaction", "emoji"), ("target_author", "targetAuthor"), ("timestamp", "targetTimestamp")];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/reactions/{number}", post(send_reaction))
        .route("/v1/reactions/{number}", delete(remove_reaction))
}

/// POST /v1/reactions/{number} — send a reaction to a message.
async fn send_reaction(
    State(st): State<AppState>,
    Path(number): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut params = body;
    translate_legacy_params(&mut params, LEGACY);
    params["account"] = json!(number);
    rpc_created(&st, "sendReaction", params).await
}

/// DELETE /v1/reactions/{number} — remove a reaction from a message.
///
/// signal-cli has no standalone "removeReaction" RPC method - reactions are
/// added and removed through the same `sendReaction` command, with a
/// `remove: true` flag distinguishing the two (see SendReactionCommand's
/// `--remove` flag in signal-cli). Calling a nonexistent "removeReaction"
/// method returned a JSON-RPC "Method not found" error from real signal-cli.
async fn remove_reaction(
    State(st): State<AppState>,
    Path(number): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let mut params = body;
    translate_legacy_params(&mut params, LEGACY);
    params["account"] = json!(number);
    params["remove"] = json!(true);
    rpc_no_content(&st, "sendReaction", params).await
}
