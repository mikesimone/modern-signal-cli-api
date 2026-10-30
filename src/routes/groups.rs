use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;
use super::avatar::spill_avatar_to_disk;
use super::helpers::{rpc_ok, rpc_created};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/groups/{number}", get(list_groups).post(create_group))
        .route(
            "/v1/groups/{number}/{groupid}",
            get(get_group).put(update_group).delete(delete_group),
        )
        .route(
            "/v1/groups/{number}/{groupid}/members",
            post(add_members).delete(remove_members),
        )
        .route(
            "/v1/groups/{number}/{groupid}/admins",
            post(add_admins).delete(remove_admins),
        )
        .route("/v1/groups/{number}/{groupid}/avatar", get(get_avatar))
        .route("/v1/groups/{number}/{groupid}/join", post(join_group))
        .route("/v1/groups/{number}/join", post(join_group_by_link))
        .route("/v1/groups/{number}/{groupid}/quit", post(quit_group))
        .route("/v1/groups/{number}/{groupid}/block", post(block_group))
        .route(
            "/v1/groups/{number}/{groupid}/messages",
            delete(admin_delete_message),
        )
}

// ---- List / Get -----------------------------------------------------------

async fn list_groups(
    State(st): State<AppState>,
    Path(number): Path<String>,
) -> Response {
    rpc_ok(&st, "listGroups", json!({ "account": number })).await
}

async fn get_group(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
) -> Response {
    rpc_ok(&st, "listGroups", json!({ "account": number, "group-id": groupid })).await
}

// ---- Create / Update / Delete ---------------------------------------------

#[derive(Deserialize)]
struct CreateGroupBody {
    name: String,
    members: Vec<String>,
    description: Option<String>,
    #[serde(default)]
    permissions: Option<GroupPermissions>,
}

#[derive(Deserialize)]
struct GroupPermissions {
    add_members: Option<String>,
    edit_details: Option<String>,
}

async fn create_group(
    State(st): State<AppState>,
    Path(number): Path<String>,
    Json(body): Json<CreateGroupBody>,
) -> Response {
    let mut params = json!({
        "account": number,
        "name": body.name,
        "member": body.members,
    });
    if let Some(desc) = &body.description {
        params["description"] = json!(desc);
    }
    if let Some(perms) = &body.permissions {
        if let Some(add) = &perms.add_members {
            params["set-permission-add-member"] = json!(add);
        }
        if let Some(edit) = &perms.edit_details {
            params["set-permission-edit-details"] = json!(edit);
        }
    }
    rpc_created(&st, "updateGroup", params).await
}

#[derive(Deserialize)]
struct UpdateGroupBody {
    name: Option<String>,
    description: Option<String>,
    base64_avatar: Option<String>,
    expiration: Option<u64>,
    #[serde(default)]
    permissions: Option<GroupPermissions>,
}

async fn update_group(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<UpdateGroupBody>,
) -> Response {
    let mut params = json!({
        "account": number,
        "group-id": groupid,
    });
    if let Some(name) = &body.name {
        params["name"] = json!(name);
    }
    if let Some(desc) = &body.description {
        params["description"] = json!(desc);
    }
    let _spilled = if let Some(avatar) = &body.base64_avatar {
        match spill_avatar_to_disk(avatar) {
            Ok((path, guard)) => {
                params["avatar"] = json!(path.to_string_lossy());
                Some(guard)
            }
            Err(e) => {
                return (StatusCode::BAD_REQUEST, Json(json!({ "error": e.to_string() }))).into_response();
            }
        }
    } else {
        None
    };
    if let Some(exp) = body.expiration {
        params["expiration"] = json!(exp);
    }
    if let Some(perms) = &body.permissions {
        if let Some(add) = &perms.add_members {
            params["set-permission-add-member"] = json!(add);
        }
        if let Some(edit) = &perms.edit_details {
            params["set-permission-edit-details"] = json!(edit);
        }
    }
    rpc_ok(&st, "updateGroup", params).await
}

async fn delete_group(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
) -> Response {
    rpc_ok(&st, "quitGroup", json!({ "account": number, "group-id": groupid, "delete": true })).await
}

// ---- Members / Admins -----------------------------------------------------

#[derive(Deserialize)]
struct MembersBody {
    members: Vec<String>,
}

async fn add_members(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<MembersBody>,
) -> Response {
    rpc_ok(&st, "updateGroup", json!({
        "account": number,
        "group-id": groupid,
        "member": body.members,
    })).await
}

async fn remove_members(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<MembersBody>,
) -> Response {
    rpc_ok(&st, "updateGroup", json!({
        "account": number,
        "group-id": groupid,
        "removeMember": body.members,
    })).await
}

#[derive(Deserialize)]
struct AdminsBody {
    admins: Vec<String>,
}

async fn add_admins(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<AdminsBody>,
) -> Response {
    rpc_ok(&st, "updateGroup", json!({
        "account": number,
        "group-id": groupid,
        "admin": body.admins,
    })).await
}

async fn remove_admins(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<AdminsBody>,
) -> Response {
    rpc_ok(&st, "updateGroup", json!({
        "account": number,
        "group-id": groupid,
        "removeAdmin": body.admins,
    })).await
}

// ---- Avatar / Join / Quit / Block -----------------------------------------

/// GET /v1/groups/{number}/{groupid}/avatar — retrieve a group's avatar,
/// base64 encoded, via signal-cli's `getAvatar` RPC method.
async fn get_avatar(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
) -> Response {
    rpc_ok(&st, "getAvatar", json!({ "account": number, "group-id": groupid })).await
}

/// POST /v1/groups/{number}/{group}/join - body `{"uri": "https://signal.group/#..."}`.
/// signal-cli can only join through a group invite link (joinGroup takes `uri`, not a group id), so the link is
/// required; the `{group}` path segment is kept for compatibility and not used.
async fn join_group(
    State(st): State<AppState>,
    Path((number, _groupid)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Response {
    join_with_body(&st, &number, &body).await
}

/// POST /v1/groups/{number}/join - same, without the unused group path segment.
async fn join_group_by_link(
    State(st): State<AppState>,
    Path(number): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    join_with_body(&st, &number, &body).await
}

async fn join_with_body(st: &AppState, number: &str, body: &[u8]) -> Response {
    let uri = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("uri").and_then(|u| u.as_str()).map(str::to_owned));
    match uri {
        Some(uri) if !uri.is_empty() => rpc_ok(st, "joinGroup", json!({ "account": number, "uri": uri })).await,
        _ => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "error": "signal-cli can only join a group through its invite link: send {\"uri\": \"https://signal.group/#...\"}" })),
        )
            .into_response(),
    }
}

async fn quit_group(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
) -> Response {
    rpc_ok(&st, "quitGroup", json!({ "account": number, "group-id": groupid })).await
}

async fn block_group(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
) -> Response {
    rpc_ok(&st, "block", json!({ "account": number, "group-id": groupid })).await
}

// ---- Admin delete -----------------------------------------------------

#[derive(Deserialize)]
struct AdminDeleteBody {
    target_author: String,
    target_timestamp: i64,
    #[serde(default)]
    story: Option<bool>,
}

/// DELETE /v1/groups/{number}/{groupid}/messages — admin-delete a message
/// in this group (removes it for all members; requires this account to be
/// a group admin). signal-cli's `sendAdminDelete` RPC method.
async fn admin_delete_message(
    State(st): State<AppState>,
    Path((number, groupid)): Path<(String, String)>,
    Json(body): Json<AdminDeleteBody>,
) -> Response {
    let mut params = json!({
        "account": number,
        "group-id": [groupid],
        "target-author": body.target_author,
        "target-timestamp": body.target_timestamp,
    });
    if let Some(true) = body.story {
        params["story"] = json!(true);
    }
    rpc_ok(&st, "sendAdminDelete", params).await
}
