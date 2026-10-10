#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod icons;
use std::{net::{Ipv4Addr, TcpListener}, path::{Path, PathBuf}, process::Command};
use serde::Serialize;
use tauri::Manager;
use webcask_core::{AppKind, InstalledApp, get_app};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppListing {
    #[serde(flatten)]
    app: InstalledApp,
    icon_data: Option<String>,
}
#[tauri::command]
fn list_apps() -> Result<Vec<AppListing>, String> {
    Ok(webcask_core::list_apps()?.into_iter().map(|app| {
        let icon_data = icons::icon_data(&app.id);
        AppListing { app, icon_data }
    }).collect())
}
#[tauri::command]
async fn install_website(name: String, url: String) -> Result<InstalledApp, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = webcask_core::install_website(&name, &url)?;
        // Icon lookup must never prevent installation of a legitimate site.
        let _ = icons::fetch_website_icon(&app);
        Ok(app)
    }).await.map_err(|e| format!("Website install task failed: {e}"))?
}
#[tauri::command]
async fn import_game(name: String, path: String) -> Result<InstalledApp, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = webcask_core::import_game(&name, Path::new(&path))?;
        let _ = icons::detect_local_icon(&app);
        Ok(app)
    }).await.map_err(|e| format!("Game import task failed: {e}"))?
}
#[tauri::command]
async fn set_app_icon(id: String, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = icons::set_custom_icon(&id, Path::new(&path))?;
        if let Ok(app) = get_app(&id) { let _ = update_existing_shortcut_icon(&app); }
        Ok(result)
    }).await.map_err(|e| format!("Icon update task failed: {e}"))?
}
#[tauri::command]
async fn detect_app_icon(id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let result = icons::restore_detected_icon(&id)?;
        if let Ok(app) = get_app(&id) { let _ = update_existing_shortcut_icon(&app); }
        Ok(result)
    }).await.map_err(|e| format!("Icon discovery task failed: {e}"))?
}
#[tauri::command]
fn set_game_focus(id: String, enabled: bool) -> Result<InstalledApp, String> {
    webcask_core::set_game_focus(&id, enabled)
}
#[tauri::command]
fn set_game_options(
    id: String, game_focus: bool, window_mode: webcask_core::WindowMode,
    window_width: u32, window_height: u32,
) -> Result<InstalledApp, String> {
    webcask_core::set_game_options(&id, game_focus, window_mode, window_width, window_height)
}
#[tauri::command]
fn remove_app(id: String) -> Result<(), String> {
    webcask_core::remove_app(&id)
}
fn runtime_binary(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let sibling = std::env::current_exe().map_err(|e| e.to_string())?
        .with_file_name("webcask-runtime.exe");
    if sibling.is_file() { return Ok(sibling); }
    let packaged = app.path().resource_dir().map_err(|e| e.to_string())?
        .join("resources").join("webcask-runtime.exe");
    if packaged.is_file() { return Ok(packaged); }
    Err("WebCask Runtime not found. Install the Windows release or compile the runtime alongside SitePocket.".into())
}
#[tauri::command]
fn launch_app(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let installed = get_app(&id)?;
    if matches!(installed.kind, AppKind::Local) {
        let site = webcask_core::app_path(&id)?.join("site");
        let entry = site.join(installed.entry_path.as_deref().unwrap_or("index.html"));
        if !entry.is_file() {
            return Err("The imported game's index.html file is missing. Remove it and import the game again.".into());
        }
        let port = installed.port.ok_or("Imported game is missing its local server port")?;
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|_| {
            format!("The game's local port {port} is already in use. Close its other window and retry.")
        })?;
    }
    Command::new(runtime_binary(&app)?).args(["--app-id", &id])
        .spawn().map_err(|e| format!("Unable to start WebCask: {e}"))?;
    Ok(())
}
#[tauri::command]
fn capture_app(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let item = get_app(&id)?;
    if !matches!(item.kind, AppKind::Online) {
        return Err("Only websites can be downloaded this way".into());
    }
    Command::new(runtime_binary(&app)?).args(["--app-id", &id, "--capture"])
        .spawn().map_err(|e| format!("Unable to open capture window: {e}"))?;
    Ok(())
}
fn shortcut_path(entry: &InstalledApp) -> Result<PathBuf, String> {
    let desktop = dirs::desktop_dir().ok_or("Cannot locate Desktop folder")?;
    let title: String = entry.name.chars().map(|c| {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }
    }).collect();
    Ok(desktop.join(format!("{}-{}.lnk", title, &entry.id[..8])))
}
fn ps_escape(s: &str) -> String { s.replace('\'', "''") }
fn execute_powershell(script: &str) -> Result<(), String> {
    let result = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output().map_err(|e| format!("PowerShell failed: {e}"))?;
    if !result.status.success() {
        return Err(format!("Windows shortcut update failed: {}", String::from_utf8_lossy(&result.stderr)));
    }
    Ok(())
}
fn update_existing_shortcut_icon(entry: &InstalledApp) -> Result<(), String> {
    let path = shortcut_path(entry)?;
    if !path.is_file() { return Ok(()); }
    let image = icons::icon_path(&entry.id)?;
    if !image.is_file() { return Ok(()); }
    let script = format!(
        "$l=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$l.IconLocation='{},0';$l.Save()",
        ps_escape(&path.to_string_lossy()), ps_escape(&image.to_string_lossy())
    );
    execute_powershell(&script)
}
#[tauri::command]
fn create_shortcut(app: tauri::AppHandle, id: String) -> Result<String, String> {
    let entry = get_app(&id)?;
    let exe = runtime_binary(&app)?;
    let destination = shortcut_path(&entry)?;
    let ico = icons::icon_path(&id)?;
    let source = if ico.is_file() { ico } else { exe.clone() };
    let script = format!(
        "$l=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$l.TargetPath='{}';$l.Arguments='--app-id {}';$l.WorkingDirectory='{}';$l.IconLocation='{},0';$l.Save()",
        ps_escape(&destination.to_string_lossy()), ps_escape(&exe.to_string_lossy()),
        entry.id, ps_escape(&exe.parent().ok_or("Runtime location invalid")?.to_string_lossy()),
        ps_escape(&source.to_string_lossy())
    );
    execute_powershell(&script)?;
    Ok(destination.to_string_lossy().to_string())
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_apps, install_website, import_game, set_app_icon, detect_app_icon,
            set_game_focus, set_game_options, remove_app, launch_app, capture_app, create_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("failed to start SitePocket");
}
