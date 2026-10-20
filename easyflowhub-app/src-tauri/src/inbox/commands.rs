//! Tauri command surface for the Inbox domain.
//!
//! Commands stay thin: they resolve the shared connection / assets directory
//! through `repository`, delegate the actual use case to `service`, and shape
//! the wire response. No command re-implements migration, attachment or
//! transaction logic.

use serde::Serialize;
use tauri_plugin_opener::OpenerExt;

use super::repository;
use super::service::{self, CaptureInboxItemInput};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemsResponse {
    pub ok: bool,
    pub count: usize,
    pub items: Vec<repository::InboxItem>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItemResponse {
    pub ok: bool,
    pub item: repository::InboxItem,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxStateResponse {
    pub ok: bool,
    pub item_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertInboxItemResponse {
    pub ok: bool,
    pub item_id: String,
    pub note_id: String,
}

#[tauri::command]
pub fn list_inbox_items(
    app: tauri::AppHandle,
    include_archived: Option<bool>,
) -> Result<InboxItemsResponse, String> {
    let conn = repository::get_connection(&app)?;
    let items = repository::list_items(&conn, include_archived.unwrap_or(false))?;
    Ok(InboxItemsResponse {
        ok: true,
        count: items.len(),
        items,
    })
}

#[tauri::command]
pub fn capture_inbox_item(
    app: tauri::AppHandle,
    input: CaptureInboxItemInput,
) -> Result<InboxItemResponse, String> {
    let item = service::capture_item(&app, input)?;
    Ok(InboxItemResponse { ok: true, item })
}

#[tauri::command]
pub fn archive_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    set_archived(&app, &id, true)
}

#[tauri::command]
pub fn restore_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    set_archived(&app, &id, false)
}

fn set_archived(app: &tauri::AppHandle, id: &str, archived: bool) -> Result<InboxStateResponse, String> {
    let conn = repository::get_connection(app)?;
    repository::set_archived(&conn, id, archived)?;
    Ok(InboxStateResponse {
        ok: true,
        item_id: id.to_string(),
    })
}

#[tauri::command]
pub fn convert_inbox_item_to_note(
    app: tauri::AppHandle,
    id: String,
) -> Result<ConvertInboxItemResponse, String> {
    let mut conn = repository::get_connection(&app)?;
    let assets_dir = repository::assets_dir(&app)?;
    let note_id = service::convert_item_to_note(&mut conn, &id, &assets_dir)?;
    Ok(ConvertInboxItemResponse { ok: true, item_id: id, note_id })
}

#[tauri::command]
pub fn open_inbox_attachment(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = repository::get_connection(&app)?;
    let filename = repository::attachment_filename(&conn, &id)?;
    let safe_name = std::path::Path::new(&filename)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| *name == filename)
        .ok_or_else(|| "附件存储路径无效。".to_string())?;
    let path = repository::assets_dir(&app)?.join(safe_name);
    open::that(path).map_err(|error| format!("打开附件失败: {error}"))
}

#[tauri::command]
pub fn open_inbox_link(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let conn = repository::get_connection(&app)?;
    let content = repository::item_link_content(&conn, &id)?;
    if !service::is_http_url(&content) {
        return Err("只允许打开 http 或 https 链接。".to_string());
    }
    open::that(content).map_err(|error| format!("打开链接失败: {error}"))
}

/// Permanently delete an Inbox item. Deletion of stored files follows the
/// resource-identity rules owned by `service::delete_item`.
#[tauri::command]
pub fn delete_inbox_item(app: tauri::AppHandle, id: String) -> Result<InboxStateResponse, String> {
    service::delete_item(&app, &id)?;
    Ok(InboxStateResponse {
        ok: true,
        item_id: id,
    })
}

/// Open the shared resource folder that stores Inbox / note attachments
/// (recordings, images and copied files) for manual management.
#[tauri::command]
pub fn open_inbox_assets_dir(app: tauri::AppHandle) -> Result<(), String> {
    let directory = repository::assets_dir(&app)?;
    app.opener()
        .open_path(directory.to_string_lossy().to_string(), None::<String>)
        .map_err(|error| format!("打开资源文件夹失败: {error}"))
}
