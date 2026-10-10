//! Native Android ContentResolver access for ZIP imports.
//! Only Rust calls the plugin; untrusted game pages cannot invoke it directly.
use tauri::{plugin::{Builder, TauriPlugin}, Runtime};

#[cfg(target_os = "android")]
use tauri::{plugin::PluginHandle, Manager};
#[cfg(target_os = "android")]
use serde::{Deserialize, Serialize};

#[cfg(target_os = "android")]
pub struct ZipBridge<R: Runtime>(PluginHandle<R>);

#[cfg(target_os = "android")]
#[derive(Serialize)]
struct StageRequest { uri: String }
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct StageResponse { path: String }

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("android-zip-import")
        .setup(|_app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api.register_android_plugin(
                    "dev.sitepocket.zipimport", "ZipImportPlugin"
                )?;
                _app.manage(ZipBridge(handle));
            }
            #[cfg(not(target_os = "android"))]
            { let _ = api; }
            Ok(())
        })
        .build()
}

/// Stage a content:// document inside the Android application's cache folder.
/// Native Kotlin chooses the destination path. The URI never becomes a Rust Path.
#[cfg(target_os = "android")]
pub fn stage_zip<R: Runtime>(app: &tauri::AppHandle<R>, uri: String)
    -> Result<std::path::PathBuf, String> {
    if !uri.starts_with("content://") {
        return Err("Select a ZIP through Android Files rather than entering a path.".into());
    }
    let response: StageResponse = app.state::<ZipBridge<R>>().0
        .run_mobile_plugin("stageZip", StageRequest { uri })
        .map_err(|e| format!("Android Files could not open the ZIP: {e}"))?;
    let file = std::path::PathBuf::from(response.path);
    if !file.is_file() || file.extension().is_none_or(|e| e != "zip") ||
       !file.file_name().is_some_and(|e| e.to_string_lossy().starts_with("sitepocket-import-")) {
        return Err("Android Files did not provide a valid staged ZIP.".into());
    }
    Ok(file)
}


#[cfg(target_os = "android")]
#[derive(Serialize)]
struct ActiveGameRequest { app_id: String }
#[cfg(target_os = "android")]
#[derive(Serialize)]
struct EmptyRequest {}
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct NativeAck { ok: bool }
#[cfg(target_os = "android")]
#[derive(Deserialize)]
struct NativeLog { content: String }

/// Records the currently launching game's UUID in app-private preferences.
/// The native Activity can associate pre-JavaScript events with this game.
#[cfg(target_os = "android")]
pub fn set_active_game<R: Runtime>(app: &tauri::AppHandle<R>, app_id: &str)
    -> Result<(), String> {
    let ack: NativeAck = app.state::<ZipBridge<R>>().0
        .run_mobile_plugin("setActiveGame",
            ActiveGameRequest { app_id: app_id.into() })
        .map_err(|e| format!("Native logger setup: {e}"))?;
    if ack.ok { Ok(()) } else { Err("Native logger setup rejected".into()) }
}

/// Native Activity / Android WebView event journal (no injected script needed).
#[cfg(target_os = "android")]
pub fn read_native_events<R: Runtime>(app: &tauri::AppHandle<R>)
    -> Result<String, String> {
    let result: NativeLog = app.state::<ZipBridge<R>>().0
        .run_mobile_plugin("readNativeEvents", EmptyRequest {})
        .map_err(|e| format!("Native WebView report unavailable: {e}"))?;
    Ok(result.content)
}

#[cfg(target_os = "android")]
pub fn clear_native_events<R: Runtime>(app: &tauri::AppHandle<R>)
    -> Result<(), String> {
    let ack: NativeAck = app.state::<ZipBridge<R>>().0
        .run_mobile_plugin("clearNativeEvents", EmptyRequest {})
        .map_err(|e| format!("Native WebView clear failed: {e}"))?;
    if ack.ok { Ok(()) } else { Err("Native WebView clear rejected".into()) }
}
