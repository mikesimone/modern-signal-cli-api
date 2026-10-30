//! Our own implementations of API routes signal-cli has no JSON-RPC method for (yet).
//!
//! Each route first tries the real signal-cli method; only when signal-cli answers "Method not implemented"
//! (JSON-RPC -32601) does it fall back to what is here. So when a newer signal-cli adds the method, the route
//! starts using it with no code change (and `contract/check.py` then flags the stale known-gaps entry, which is
//! the cue to review that the route's parameters still match).
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

/// signal-cli's data directory: SIGNAL_CLI_CONFIG, else $HOME/.local/share/signal-cli (its own default).
pub fn default_data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("SIGNAL_CLI_CONFIG") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".local/share/signal-cli")
}

// ---- attachments (signal-cli keeps received attachments as plain files in <data>/attachments) -------------

/// Attachment ids are file names signal-cli generated; refuse anything that could leave the directory.
pub fn valid_attachment_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && !id.starts_with('.')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn list_attachments(data_dir: &Path) -> io::Result<Vec<Value>> {
    let dir = data_dir.join("attachments");
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e),
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        // skip signal-cli's in-flight "<id>.<n>.tmp" downloads and anything not a regular file
        if !meta.is_file() || name.ends_with(".tmp") || !valid_attachment_id(&name) {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(json!({ "id": name, "size": meta.len(), "modified": modified }));
    }
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    Ok(out)
}

/// Ok(true) = deleted, Ok(false) = no such attachment.
pub fn delete_attachment(data_dir: &Path, id: &str) -> io::Result<bool> {
    if !valid_attachment_id(id) {
        return Ok(false);
    }
    match std::fs::remove_file(data_dir.join("attachments").join(id)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

// ---- per-account settings (signal-cli can write these but offers no way to read them back) --------------

static SETTINGS_LOCK: Mutex<()> = Mutex::new(());
const SETTINGS_FILE: &str = "modern-signal-cli-api-settings.json";

/// Settings sent through updateConfiguration (synced to linked devices).
pub const CONFIGURATION_SETTINGS: &[&str] = &["readReceipts", "typingIndicators", "linkPreviews", "unidentifiedDeliveryIndicators"];
/// Settings sent through updateAccount.
pub const ACCOUNT_SETTINGS: &[&str] = &["deviceName", "numberSharing", "discoverableByNumber", "unrestrictedUnidentifiedSender", "username", "deleteUsername"];

/// snake_case / dash-case -> camelCase, so bodies written for bbernhard's API (read_receipts) work too.
pub fn camel(key: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in key.chars() {
        if c == '_' || c == '-' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn load_settings(data_dir: &Path, account: &str) -> Value {
    let _g = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_all(data_dir).remove(account).unwrap_or_else(|| json!({}))
}

/// Merge `patch` into the account's remembered settings.
pub fn save_settings(data_dir: &Path, account: &str, patch: &Map<String, Value>) -> io::Result<()> {
    let _g = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read_all(data_dir);
    let entry = all.entry(account.to_string()).or_insert_with(|| json!({}));
    if let Some(obj) = entry.as_object_mut() {
        for (k, v) in patch {
            obj.insert(k.clone(), v.clone());
        }
    }
    std::fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!("{SETTINGS_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&all)?)?;
    std::fs::rename(tmp, data_dir.join(SETTINGS_FILE))
}

fn read_all(data_dir: &Path) -> BTreeMap<String, Value> {
    std::fs::read(data_dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
