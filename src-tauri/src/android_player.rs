//! Android WebCask player: independent Android WebView window.
//! Tauri capabilities apply only to "main": remote games get no native commands.
use tauri::{AppHandle, Manager, WebviewUrl, webview::WebviewWindowBuilder};
use webcask_core::AppKind;

pub fn launch(app: AppHandle, id: String) -> Result<(), String> {
    let item = webcask_core::get_app(&id)?;
    if !matches!(item.kind, AppKind::Online) {
        return Err("Imported offline games aren't available in the Android player yet. Android currently supports online games; ZIP playback needs a mobile local-asset server.".into());
    }
    let address = item.url.as_deref().ok_or("Website URL missing")?;
    let url = url::Url::parse(address).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err("Only HTTP(S) games can be launched.".into());
    }
    let label = format!("webcask-{}", item.id);
    if app.get_webview_window(&label).is_some() {
        return Err("This game is already open. Return to its player window.".into());
    }
    let origin = url.origin().ascii_serialization();
    let script = include_str!("android_touch.js")
        .replace("__ALLOWED_ORIGIN__", &serde_json::to_string(&origin).map_err(|e| e.to_string())?)
        .replace("__BUTTONS_JSON__", &serde_json::to_string(&item.touch_buttons).map_err(|e| e.to_string())?)
        .replace("__ENABLED__", if item.touch_enabled { "true" } else { "false" });
    let mut builder = WebviewWindowBuilder::new(&app, label, WebviewUrl::External(url))
        .title(&item.name)
        .initialization_script(script);
    if item.game_focus {
        builder = builder.initialization_script(include_str!("../../crates/webcask-runtime/src/game_focus.js"));
    }
    builder.build().map_err(|e| format!(
        "Could not open Android WebCask player: {e}. Multiple player windows require Android 12L or later."
    ))?;
    Ok(())
}