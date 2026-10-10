//! WebCask diagnostics: bounded, persisted runtime event history.
//! Diagnostic reports survive an app restart after a WebView hang/crash.
//! They contain errors and operational phases, not cookies, request bodies
//! or arbitrary web-page content beyond the error message/stack.
use serde::Serialize;
use std::{
    collections::HashMap, fs,
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use webcask_core::get_app;

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub id: u64,
    pub app_id: String,
    pub app_name: String,
    pub time: u64,
    pub severity: String, // "info", "warning", "error"
    pub source: String,   // "runtime", "game", "assets"
    pub code: String,
    pub message: String,
    pub detail: String,
}
#[derive(Default)]
struct State {
    history: Vec<Diagnostic>,
    current_id: u64,
    started: HashMap<String, (u64, Instant, bool)>,
}
static STATE: OnceLock<Mutex<State>> = OnceLock::new();
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis() as u64
}
fn file() -> Result<std::path::PathBuf, String> {
    Ok(webcask_core::root()?.join("webcask-diagnostics.json"))
}
fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| {
        let history = file().ok().and_then(|p| fs::read(p).ok())
            .filter(|data| data.len() <= 512 * 1024)
            .and_then(|data| serde_json::from_slice::<Vec<Diagnostic>>(&data).ok())
            .unwrap_or_default();
        let current_id = history.iter().map(|event| event.id).max().unwrap_or(0);
        Mutex::new(State { history, current_id, started: HashMap::new() })
    })
}
fn limit(s: impl AsRef<str>, max: usize) -> String {
    s.as_ref().chars().filter(|c| !c.is_control() || *c == '\n')
        .take(max).collect()
}
fn push(item: &mut State, app_id: &str, severity: &str,
        source: &str, code: &str, message: &str, detail: &str) {
    item.current_id += 1;
    let name = get_app(app_id).ok().map(|app| app.name)
        .unwrap_or_else(|| "Unknown game".to_owned());
    item.history.push(Diagnostic {
        id: item.current_id, app_id: app_id.to_owned(), app_name: name,
        time: now(), severity: severity.into(), source: source.into(),
        code: code.into(), message: limit(message, 500), detail: limit(detail, 1800),
    });
    if item.history.len() > 100 {
        let excess = item.history.len() - 100;
        item.history.drain(..excess);
    }
    // Info is also persisted (bounded) for diagnosing hard freezes where the
    // JS runtime never sends the next stage.
    if let Ok(path) = file() {
        if let Ok(bytes) = serde_json::to_vec(&item.history) {
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, bytes).is_ok() {
                let _ = fs::rename(tmp, path);
            }
        }
    }
}
pub fn record(app_id: &str, severity: &str, source: &str, code: &str,
    message: &str, detail: &str) {
    if let Ok(mut item) = state().lock() {
        push(&mut item, app_id, severity, source, code, message, detail);
    }
}
pub fn begin(app_id: &str) {
    let generation = now();
    if let Ok(mut s) = state().lock() {
        s.started.insert(app_id.into(), (generation, Instant::now(), false));
        push(&mut s, app_id, "info", "runtime", "launch-started",
            "Preparing WebCask Android player", "Waiting for the WebView to load.");
    }
    let id = app_id.to_owned();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(22));
        let overdue = state().lock().ok()
            .and_then(|mut s| s.started.get_mut(&id).map(|(gen, t, ready)| {
                *gen == generation && !*ready && t.elapsed() >= Duration::from_secs(22)
            })).unwrap_or(false);
        if overdue {
            record(&id, "error", "runtime", "startup-timeout",
                "Possible WebCask startup freeze",
                "No page-ready signal arrived within 22 seconds. This is not proof the game is broken: the Android WebView or window creation may have stalled, or the page may have blocked the diagnostic script.");
        }
    });
}
pub fn ready(app_id: &str, phase: &str) {
    if let Ok(mut s) = state().lock() {
        if let Some(v) = s.started.get_mut(app_id) { v.2 = true; }
        push(&mut s, app_id, "info", "runtime", phase,
            "Game document became ready", "WebCask received a game-page readiness signal.");
    }
}
pub fn list(since: u64) -> Vec<Diagnostic> {
    state().lock().map(|s| {
        s.history.iter().filter(|item| item.id > since).cloned().collect()
    }).unwrap_or_default()
}
pub fn clear() {
    if let Ok(mut s) = state().lock() {
        s.history.clear();
        // Keep monotonically increasing IDs so polling clients do not miss
        // events after the user clears the previous report.
        if let Ok(p) = file() { let _ = fs::remove_file(p); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanitizes_untrusted_messages() {
        assert_eq!(limit("hello\nworld", 20), "hello\nworld");
        assert_eq!(limit("x\0y", 20), "xy");
        assert_eq!(limit("abcdef", 3), "abc");
    }
}
