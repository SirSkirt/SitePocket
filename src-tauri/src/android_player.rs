//! Android WebCask player: independent Android WebView window.
//! Tauri capabilities apply only to "main": remote games get no native commands.
use tauri::{AppHandle, Manager, WebviewUrl, webview::{WebviewWindowBuilder, PageLoadEvent}};
use webcask_core::AppKind;

pub fn launch(app: AppHandle, id: String) -> Result<(), String> {
    let item = webcask_core::get_app(&id)?;
    // WebCask Mobile serves imported games from a read-only localhost server,
    // not file://, so fetch(), workers, ES modules and WASM MIME work.
    // This loopback address remains available with airplane mode enabled.
    let address = match &item.kind {
        AppKind::Online => item.url.clone().ok_or("Website URL missing")?,
        AppKind::Local => super::mobile_server::serve(&item)?,
    };
    let url = url::Url::parse(&address).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err("Only HTTP(S) games can be launched.".into());
    }
    let label = format!("webcask-{}", item.id);
    if app.get_webview_window(&label).is_some() {
        return Err("This game is already open. Return to its player window.".into());
    }
    let origin = url.origin().ascii_serialization();
    let debug_script = if matches!(item.kind, AppKind::Local) {
        Some(include_str!("android_debug.js")
            .replace("__WEBCASK_ALLOWED_ORIGIN__",
                &serde_json::to_string(&origin).map_err(|e| e.to_string())?))
    } else { None };
    let script = include_str!("android_touch.js")
        .replace("__ALLOWED_ORIGIN__", &serde_json::to_string(&origin).map_err(|e| e.to_string())?)
        .replace("__BUTTONS_JSON__", &serde_json::to_string(&item.touch_buttons).map_err(|e| e.to_string())?)
        .replace("__ENABLED__", if item.touch_enabled { "true" } else { "false" });
    // The Android window builder must be parented to the existing main
    // activity on multi-window capable devices (API 32+).
    let main = app.get_webview_window("main")
        .ok_or("SitePocket's main Android window is unavailable")?;
    crate::diagnostics::begin(&item.id);
    let launch_id = item.id.clone();
    let mut builder = WebviewWindowBuilder::new(&main, label, WebviewUrl::External(url))
        .title(&item.name)
        .on_page_load(move |_window, payload| {
            let phase = match payload.event() {
                PageLoadEvent::Started => "webview-navigation-started",
                PageLoadEvent::Finished => "webview-navigation-finished",
            };
            crate::diagnostics::record(&launch_id, "info", "runtime", phase,
                "WebView navigation event", &payload.url().to_string());
        })
        .initialization_script(script);
    if let Some(js) = debug_script {
        builder = builder.initialization_script(js);
    }
    if item.game_focus {
        builder = builder.initialization_script(include_str!("../../crates/webcask-runtime/src/game_focus.js"));
    }
    builder.build().map_err(|e| {
        crate::diagnostics::record(&item.id, "error", "runtime", "webview-create-failed",
            "Android could not create the game WebView", &e.to_string());
        format!("Could not open Android WebCask player: {e}. Android multi-window behavior may depend on the device and OS.")
    })?;
    crate::diagnostics::record(&item.id, "info", "runtime", "webview-created",
        "Android created the game WebView", "Waiting for the page to load.");
    Ok(())
}