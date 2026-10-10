//! Android WebCask player: independent Android WebView window.
//! Tauri capabilities apply only to "main": remote games get no native commands.
use tauri::{AppHandle, Manager, WebviewUrl, webview::{WebviewWindowBuilder, PageLoadEvent}};
use webcask_core::AppKind;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn hook_confirmed(journal: &str, app_id: &str, after_ms: u64) -> bool {
    journal.lines().filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|event| event.get("appId").and_then(|v| v.as_str()) == Some(app_id)
            && event.get("code").and_then(|v| v.as_str()) == Some("native-console-hook-installed")
            && event.get("time").and_then(|v| v.as_u64()).unwrap_or(0) >= after_ms)
}

pub fn launch(app: AppHandle, id: String, native_only: bool) -> Result<(), String> {
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
    let debug_script = if !native_only && matches!(item.kind, AppKind::Local) {
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
    // Tie native Kotlin lifecycle events to the launching game before
    // requesting the secondary Activity. Errors here do not prevent launch.
    if let Err(error) = tauri_plugin_android_zip_import::set_active_game(&app, &item.id) {
        crate::diagnostics::record(&item.id, "warning", "native-webview",
            "native-logger-setup-failed", "Could not initialize native WebView logger", &error);
    }
    crate::diagnostics::begin(&item.id);
    crate::diagnostics::record(&item.id, "info", "runtime",
        "injection-mode", "WebCask page instrumentation mode",
        if native_only { "Native-only test: no WebCask initialization JavaScript (touch, focus or page debugger)." }
        else { "Normal: Android touch, local page debugger and optional focus may inject JavaScript." });
    let launch_id = item.id.clone();
    let expected_origin = origin.clone();
    // Wry creates its initial WebView before installing the native
    // WebChromeClient, so hooking the Activity alone cannot guarantee
    // capture of the first navigation. Start at a blank document, wait for
    // the Kotlin hook acknowledgement, THEN navigate to the game.
    let blank = url::Url::parse("about:blank").map_err(|e| e.to_string())?;
    let mut builder = WebviewWindowBuilder::new(&main, label, WebviewUrl::External(blank))
        // Android needs a registered TauriActivity subclass for every extra
        // window. Without it, activity launch may stall before WebView exists.
        .activity_name("WebCaskActivity")
        .title(&item.name)
        .on_page_load(move |_window, payload| {
            // Ignore the empty bootstrap document: it is not game readiness.
            if payload.url().origin().ascii_serialization() != expected_origin { return; }
            let phase = match payload.event() {
                PageLoadEvent::Started => "webview-navigation-started",
                PageLoadEvent::Finished => "webview-navigation-finished",
            };
            crate::diagnostics::record(&launch_id, "info", "runtime", phase,
                "WebView navigation event",
                payload.url().to_string().split(['?', '#']).next().unwrap_or(""));
            // A finished navigation is itself evidence that Android rendered
            // the page, even if ContentResolver / script injection is blocked.
            // Mark it ready so the watchdog doesn't falsely flag a healthy
            // online site or a game whose injected JS is disabled.
            if matches!(payload.event(), PageLoadEvent::Finished) {
                crate::diagnostics::ready(&launch_id, "webview-navigation-ready");
            }
        })
        ;
    if !native_only {
        builder = builder.initialization_script(script);
    }
    if let Some(js) = debug_script {
        builder = builder.initialization_script(js);
    }
    if item.game_focus && !native_only {
        builder = builder.initialization_script(include_str!("../../crates/webcask-runtime/src/game_focus.js"));
    }
    crate::diagnostics::record(&item.id, "info", "runtime",
        "android-activity-build-requested", "Requesting WebCask player Activity",
        "Creating the registered WebCaskActivity. A timeout after this stage points to Android Activity / window startup.");
    let started_at_ms = SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis() as u64;
    let window = builder.build().map_err(|e| {
        crate::diagnostics::record(&item.id, "error", "runtime", "webview-create-failed",
            "Android could not create the game WebView", &e.to_string());
        format!("Could not open Android WebCask player: {e}. Android multi-window behavior may depend on the device and OS.")
    })?;
    crate::diagnostics::record(&item.id, "info", "runtime", "webview-created",
        "Android created the bootstrap WebView", "Waiting for native console capture before game navigation.");

    let deadline = Instant::now() + Duration::from_secs(7);
    let mut hook_ready = false;
    while Instant::now() < deadline {
        if let Ok(journal) = tauri_plugin_android_zip_import::read_native_events(&app) {
            if hook_confirmed(&journal, &item.id, started_at_ms) {
                hook_ready = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(60));
    }
    if !hook_ready {
        crate::diagnostics::record(&item.id, "error", "native-webview",
            "native-console-hook-timeout",
            "Native console capture was not confirmed before game navigation",
            "The player was not allowed to load the game without its native console hook. Review the native Activity log.");
        let _ = window.close();
        return Err("Native WebView console hook could not be confirmed. Game navigation was cancelled for diagnostics.".into());
    }
    crate::diagnostics::record(&item.id, "info", "native-webview",
        "native-console-before-game",
        "Native console capture confirmed BEFORE game navigation",
        "Wry WebChromeClient has been wrapped. Now navigating from about:blank to the game origin.");
    window.navigate(url).map_err(|e| {
        crate::diagnostics::record(&item.id, "error", "runtime",
            "webview-navigation-failed", "Could not load game after native console hook", &e.to_string());
        format!("Could not navigate WebCask player to the game: {e}")
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hook_requires_matching_game_time_and_code() {
        let journal = concat!(
            "{\"appId\":\"first\",\"code\":\"native-console-hook-installed\",\"time\":110}\\n",
            "{\"appId\":\"second\",\"code\":\"native-console-hook-installed\",\"time\":210}\\n");
        assert!(hook_confirmed(journal, "second", 200));
        assert!(!hook_confirmed(journal, "first", 200));
        assert!(!hook_confirmed(journal, "third", 0));
    }
}