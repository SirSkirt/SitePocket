//! Application registry and game importer; no Tauri or browser privileges.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::{Path, PathBuf}, sync::OnceLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppKind { Online, Local }

/// Launch-time window presentation; automatically defaults for older app registries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}
/// Each game may have its own screen buttons; mappings are intentionally
/// saved in the shared registry so Android and future platforms can use them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchButton {
    pub id: String,
    pub label: String,
    pub key: String,
    pub x: f32,
    pub y: f32,
    pub size: u8,
}

pub fn default_touch_buttons() -> Vec<TouchButton> {
    [
        ("up", "▲", "ArrowUp", 18.0, 63.0),
        ("left", "◀", "ArrowLeft", 8.0, 75.0),
        ("down", "▼", "ArrowDown", 18.0, 85.0),
        ("right", "▶", "ArrowRight", 28.0, 75.0),
        ("a", "A", "KeyZ", 86.0, 75.0),
        ("b", "B", "KeyX", 73.0, 85.0),
        ("start", "START", "Enter", 50.0, 91.0),
    ].into_iter().map(|(id, label, key, x, y)| TouchButton {
        id: id.into(), label: label.into(), key: key.into(), x, y, size: 56,
    }).collect()
}
fn default_touch_enabled() -> bool { true }
pub fn validate_touch_buttons(buttons: &[TouchButton]) -> Result<(), String> {
    if buttons.len() > 24 { return Err("A game supports at most 24 touch controls.".into()); }
    let mut ids = std::collections::HashSet::new();
    for button in buttons {
        if button.id.is_empty() || button.id.len() > 32 ||
           !button.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') ||
           !ids.insert(button.id.as_str()) {
            return Err("Each touch button needs a unique alphanumeric ID.".into());
        }
        if button.label.trim().is_empty() || button.label.chars().count() > 16 ||
           button.label.chars().any(|c| c.is_control()) {
            return Err("Touch button labels must be 1–16 visible characters.".into());
        }
        if !valid_key(&button.key) {
            return Err(format!("Unsupported keyboard mapping: {}.", button.key));
        }
        if !button.x.is_finite() || !button.y.is_finite() ||
           !(4.0..=96.0).contains(&button.x) || !(4.0..=96.0).contains(&button.y) ||
           !(32..=96).contains(&button.size) {
            return Err("Touch position must be 4–96%, with buttons 32–96 px wide.".into());
        }
    }
    Ok(())
}
fn valid_key(key: &str) -> bool {
    matches!(key, "ArrowUp"|"ArrowDown"|"ArrowLeft"|"ArrowRight"|
        "Space"|"Enter"|"Escape"|"Tab"|"ShiftLeft"|"ControlLeft"|
        "AltLeft"|"Backspace") ||
        (key.len() == 4 && key.starts_with("Key") &&
         key.as_bytes()[3].is_ascii_uppercase()) ||
        (key.len() == 6 && key.starts_with("Digit") &&
         key.as_bytes()[5].is_ascii_digit())
}
fn default_window_width() -> u32 { 1150 }
fn default_window_height() -> u32 { 760 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub kind: AppKind,
    pub url: Option<String>,
    pub port: Option<u16>,
    #[serde(default)]
    pub game_focus: bool,
    /// Relative HTML entry point, e.g. "dist/index.html". Old apps default to root.
    #[serde(default)]
    pub entry_path: Option<String>,
    #[serde(default)]
    pub window_mode: WindowMode,
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    #[serde(default = "default_window_height")]
    pub window_height: u32,
    #[serde(default = "default_touch_enabled")]
    pub touch_enabled: bool,
    #[serde(default = "default_touch_buttons")]
    pub touch_buttons: Vec<TouchButton>,
}
static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();
/// Mobile data paths must come from the Tauri platform API; dirs returns
/// no reliable Android app-data path. Initialize once in the app setup hook.
pub fn configure_root(path: PathBuf) -> Result<(), String> {
    if let Some(previous) = DATA_ROOT.get() {
        if previous == &path { return Ok(()); }
        return Err("App-data path is already configured".into());
    }
    DATA_ROOT.set(path).map_err(|_| "Could not configure app-data path".to_string())
}
pub fn root() -> Result<PathBuf, String> {
    let path = match DATA_ROOT.get() {
        Some(path) => path.clone(),
        None => dirs::data_local_dir().ok_or("Local application data directory unavailable")?.join("SitePocket"),
    };
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn registry() -> Result<PathBuf, String> { Ok(root()?.join("apps.json")) }
pub fn app_path(id: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    Ok(root()?.join("apps").join(id))
}
pub fn profile_path(id: &str) -> Result<PathBuf, String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    Ok(root()?.join("profiles").join(id))
}
pub fn list_apps() -> Result<Vec<InstalledApp>, String> {
    let path = registry()?;
    if !path.exists() { return Ok(Vec::new()); }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Cannot read app registry: {e}"))
}
fn save_apps(apps: &[InstalledApp]) -> Result<(), String> {
    let content = serde_json::to_vec_pretty(apps).map_err(|e| e.to_string())?;
    fs::write(registry()?, content).map_err(|e| e.to_string())
}
fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 { return Err("Name must be 1–80 characters".into()); }
    Ok(name.to_owned())
}
pub fn install_website(name: &str, address: &str) -> Result<InstalledApp, String> {
    let name = checked_name(name)?;
    let parsed = url::Url::parse(address.trim()).map_err(|_| "Enter a full http:// or https:// URL")?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none()
        || !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Only HTTP(S) websites without embedded credentials are supported".into());
    }
    let app = InstalledApp { id: Uuid::new_v4().to_string(), name, kind: AppKind::Online,
        url: Some(parsed.to_string()), port: None, game_focus: false, entry_path: None, window_mode: WindowMode::default(),
        window_width: default_window_width(), window_height: default_window_height(),
        touch_enabled: true, touch_buttons: default_touch_buttons() };
    let mut apps = list_apps()?;
    apps.push(app.clone());
    save_apps(&apps)?;
    Ok(app)
}
const MAX_GAME_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_GAME_FILES: usize = 20_000;

/// Import first, validate second, then publish in the registry.
/// Source files and the original archive are never modified.
pub fn import_game(name: &str, source: &Path) -> Result<InstalledApp, String> {
    let name = checked_name(name)?;
    if !source.exists() {
        return Err("Game source not found. Re-select the folder or ZIP file.".into());
    }
    let id = Uuid::new_v4().to_string();
    let install_root = app_path(&id)?;
    let target = install_root.join("site");
    if source.is_dir() {
        let canonical = source.canonicalize().map_err(|e| format!("Can't open game folder: {e}"))?;
        let destination = root()?.canonicalize().map_err(|e| e.to_string())?;
        if destination.starts_with(&canonical) {
            return Err("That folder contains SitePocket's own data. Select the specific game folder instead.".into());
        }
    }
    fs::create_dir_all(&target).map_err(|e| format!("Can't create game directory: {e}"))?;
    let result = (|| {
        let copied = import_files(source, &target)?;
        let entry = find_game_entry(&target)?;
        let mut apps = list_apps()?;
        let taken: Vec<u16> = apps.iter().filter_map(|a| a.port).collect();
        let port = (41000..61000).find(|p| !taken.contains(p))
            .ok_or("No free local game port is available")?;
        let entry_path = relative_url_path(&entry)?;
        let app = InstalledApp {
            id: id.clone(), name, kind: AppKind::Local, url: None, port: Some(port),
            game_focus: false, entry_path: Some(entry_path),
            window_mode: WindowMode::default(),
            window_width: default_window_width(), window_height: default_window_height(),
            touch_enabled: true, touch_buttons: default_touch_buttons(),
        };
        if copied == 0 { return Err("The selected game folder or ZIP is empty.".into()); }
        apps.push(app.clone());
        save_apps(&apps)?;
        Ok(app)
    })();
    if result.is_err() { let _ = fs::remove_dir_all(install_root); }
    result
}

fn relative_url_path(path: &Path) -> Result<String, String> {
    let parts = path.components().map(|component| match component {
        std::path::Component::Normal(s) => s.to_str().map(str::to_owned),
        _ => None,
    }).collect::<Option<Vec<_>>>()
        .ok_or("Game entry point contains an unsupported file name")?;
    if parts.is_empty() { return Err("Could not locate the game entry point".into()); }
    Ok(parts.join("/"))
}

/// Prefer index.html at the root, otherwise accept exactly one nested candidate.
/// Avoid guessing when a package contains demos, samples and multiple games.
fn find_game_entry(root: &Path) -> Result<PathBuf, String> {
    let mut candidates = Vec::<PathBuf>::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| format!("Could not inspect imported game: {e}"))?;
        if !entry.file_type().is_file() { continue; }
        if !entry.file_name().to_string_lossy().eq_ignore_ascii_case("index.html") { continue; }
        let rel = entry.path().strip_prefix(root).map_err(|e| e.to_string())?.to_owned();
        if rel.components().count() <= 6 {
            candidates.push(rel);
        }
    }
    if candidates.is_empty() {
        return Err("No index.html found. Choose an exported HTML5 game folder or ZIP (not its source-code project).".into());
    }
    candidates.sort_by_key(|p| (p.components().count(), p.to_string_lossy().to_lowercase()));
    if candidates[0].components().count() == 1 { return Ok(candidates.remove(0)); }
    if candidates.len() > 1 {
        let examples = candidates.iter().take(3)
            .map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ");
        return Err(format!("Multiple games found ({examples}). Choose the folder containing the index.html you want."));
    }
    Ok(candidates.remove(0))
}

fn import_files(source: &Path, target: &Path) -> Result<usize, String> {
    let mut total = 0_u64;
    let mut file_count = 0_usize;
    if source.is_dir() {
        let source = source.canonicalize().map_err(|e| format!("Cannot open game folder: {e}"))?;
        for entry in walkdir::WalkDir::new(&source).follow_links(false) {
            let entry = entry.map_err(|e| format!("Cannot inspect game files: {e}"))?;
            if entry.file_type().is_symlink() {
                return Err("The selected folder contains symbolic links, which are not supported.".into());
            }
            let rel = entry.path().strip_prefix(&source).map_err(|e| e.to_string())?;
            let dest = target.join(rel);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&dest).map_err(|e| format!("Can't create game subfolder: {e}"))?;
                continue;
            }
            if !entry.file_type().is_file() { continue; }
            file_count += 1;
            if file_count > MAX_GAME_FILES { return Err("Import contains over 20,000 files. Choose a smaller game export.".into()); }
            let size = entry.metadata().map_err(|e| e.to_string())?.len();
            total = total.checked_add(size).ok_or("Game size overflow")?;
            if total > MAX_GAME_BYTES { return Err("The game exceeds the 2 GiB import limit.".into()); }
            if let Some(parent) = dest.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            fs::copy(entry.path(), dest).map_err(|e| format!("Unable to copy {}: {e}", rel.display()))?;
        }
    } else if source.is_file() && source.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) {
        let file = fs::File::open(source).map_err(|e| format!("Cannot open ZIP: {e}"))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| format!("Could not read ZIP archive ({e}). Check that it's a valid ZIP file."))?;
        if archive.len() > MAX_GAME_FILES { return Err("ZIP contains over 20,000 files.".into()); }
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| format!("Cannot read ZIP entry #{i}: {e}"))?;
            if entry.is_symlink() {
                return Err("ZIP contains symbolic links; importing them is not supported.".into());
            }
            let relative = entry.enclosed_name()
                .ok_or("ZIP contains a path outside the game folder; extraction was cancelled.")?
                .to_owned();
            let dest = target.join(&relative);
            if entry.is_dir() {
                fs::create_dir_all(&dest).map_err(|e| format!("Can't create ZIP subfolder: {e}"))?;
                continue;
            }
            file_count += 1;
            if file_count > MAX_GAME_FILES { return Err("ZIP contains over 20,000 files.".into()); }
            total = total.checked_add(entry.size()).ok_or("Game size overflow")?;
            if total > MAX_GAME_BYTES { return Err("The uncompressed ZIP exceeds the 2 GiB import limit.".into()); }
            if let Some(parent) = dest.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            let mut out = fs::File::create(&dest)
                .map_err(|e| format!("Cannot write {}: {e}", relative.display()))?;
            let written = io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Could not extract {}: {e}", relative.display()))?;
            if written != entry.size() { return Err(format!("ZIP entry {} was incomplete.", relative.display())); }
        }
    } else {
        return Err("Choose a game folder or a .zip archive, not a single file.".into());
    }
    Ok(file_count)
}

/// Persist a per-game mapping; does not touch game save data or files.
pub fn set_touch_controls(id: &str, enabled: bool, buttons: Vec<TouchButton>)
    -> Result<InstalledApp, String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    validate_touch_buttons(&buttons)?;
    let mut apps = list_apps()?;
    let item = apps.iter_mut().find(|app| app.id == id).ok_or("Application not found")?;
    item.touch_enabled = enabled;
    item.touch_buttons = buttons;
    let result = item.clone();
    save_apps(&apps)?;
    Ok(result)
}
/// Validate launch settings here, not just in the UI. The game window is resizable
/// after launch; these dimensions describe its initial windowed size.
pub fn validate_game_options(width: u32, height: u32) -> Result<(), String> {
    if !(480..=3840).contains(&width) || !(320..=2160).contains(&height) {
        return Err("Window size must be 480–3840 px wide and 320–2160 px high.".into());
    }
    Ok(())
}

/// Save options for an existing app without changing its files, ID, or profile.
/// The next independent WebCask launch picks up these values automatically.
pub fn set_game_options(
    id: &str, game_focus: bool, window_mode: WindowMode, window_width: u32,
    window_height: u32,
) -> Result<InstalledApp, String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    validate_game_options(window_width, window_height)?;
    let mut apps = list_apps()?;
    let entry = apps.iter_mut().find(|a| a.id == id).ok_or("Application not found")?;
    entry.game_focus = game_focus;
    entry.window_mode = window_mode;
    entry.window_width = window_width;
    entry.window_height = window_height;
    let updated = entry.clone();
    save_apps(&apps)?;
    Ok(updated)
}

/// Persist launch preferences without overwriting other app entries.
pub fn set_game_focus(id: &str, enabled: bool) -> Result<InstalledApp, String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    let mut apps = list_apps()?;
    let entry = apps.iter_mut().find(|a| a.id == id).ok_or("Application not found")?;
    entry.game_focus = enabled;
    let updated = entry.clone();
    save_apps(&apps)?;
    Ok(updated)
}
pub fn remove_app(id: &str) -> Result<(), String> {
    Uuid::parse_str(id).map_err(|_| "Invalid application ID".to_string())?;
    let mut apps = list_apps()?;
    if !apps.iter().any(|a| a.id == id) { return Err("Application not found".into()); }
    apps.retain(|a| a.id != id);
    save_apps(&apps)?;
    let _ = fs::remove_dir_all(app_path(id)?);
    let _ = fs::remove_dir_all(profile_path(id)?);
    Ok(())
}
pub fn get_app(id: &str) -> Result<InstalledApp, String> {
    list_apps()?.into_iter().find(|a| a.id == id).ok_or("Application not found".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn touch_layout_validation() {
        let mut buttons = default_touch_buttons();
        assert!(validate_touch_buttons(&buttons).is_ok());
        buttons[0].key = "KeyP".into();
        assert!(validate_touch_buttons(&buttons).is_ok());
        buttons[0].key = "system.shell".into();
        assert!(validate_touch_buttons(&buttons).is_err());
        buttons[0].key = "KeyP".into();
        buttons[0].x = f32::NAN;
        assert!(validate_touch_buttons(&buttons).is_err());
    }
    #[test]
    fn rejects_invalid_window_dimensions() {
        assert!(validate_game_options(480, 320).is_ok());
        assert!(validate_game_options(3840, 2160).is_ok());
        assert!(validate_game_options(479, 760).is_err());
        assert!(validate_game_options(1280, 2161).is_err());
    }
    #[test]
    fn modes_roundtrip() {
        let app = InstalledApp { id: Uuid::new_v4().to_string(), name: "Test".into(), kind: AppKind::Local, url: None, port: Some(42000), game_focus: true, entry_path: Some("Build/index.html".into()),
            window_mode: WindowMode::Fullscreen, window_width: 1280, window_height: 720, touch_enabled: true, touch_buttons: default_touch_buttons(), };
        let value = serde_json::to_string(&app).unwrap();
        let back: InstalledApp = serde_json::from_str(&value).unwrap();
        assert!(matches!(back.kind, AppKind::Local));
        assert!(back.game_focus);
        assert_eq!(back.entry_path.as_deref(), Some("Build/index.html"));
        assert_eq!(back.window_mode, WindowMode::Fullscreen);
        assert_eq!(back.window_width, 1280);
        assert_eq!(back.touch_buttons.len(), 7);
        assert_eq!(back.window_height, 720);
    }

    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!("webcask-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn folder_with_nested_index_works_and_preserves_assets() {
        let root = fixture();
        let original = root.join("export"); let dest = root.join("copy");
        fs::create_dir_all(original.join("Build/assets")).unwrap();
        fs::write(original.join("Build/index.html"), "<script src=\"assets/game.js\"></script>").unwrap();
        fs::write(original.join("Build/assets/game.js"), "console.log(1)").unwrap();
        assert_eq!(import_files(&original, &dest).unwrap(), 2);
        assert_eq!(relative_url_path(&find_game_entry(&dest).unwrap()).unwrap(), "Build/index.html");
        assert!(dest.join("Build/assets/game.js").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn zip_with_wrapper_dir_works() {
        use std::io::Write;
        let root = fixture();
        let source = root.join("game.zip");
        {
            let writer = fs::File::create(&source).unwrap();
            let mut zip = zip::ZipWriter::new(writer);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("Published/index.html", options).unwrap();
            zip.write_all(b"<html><script src='engine.js'></script></html>").unwrap();
            zip.start_file("Published/engine.js", options).unwrap();
            zip.write_all(b"console.log('ok')").unwrap();
            zip.finish().unwrap();
        }
        let dest = root.join("import");
        assert_eq!(import_files(&source, &dest).unwrap(), 2);
        assert_eq!(relative_url_path(&find_game_entry(&dest).unwrap()).unwrap(), "Published/index.html");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_entry_returns_diagnostic() {
        let root = fixture(); let src = root.join("src"); let dest = root.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.js"), "hello").unwrap();
        import_files(&src, &dest).unwrap();
        assert!(find_game_entry(&dest).unwrap_err().contains("No index.html"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn multiple_nested_entries_are_not_guessed() {
        let root = fixture();
        fs::create_dir_all(root.join("one")).unwrap();
        fs::create_dir_all(root.join("two")).unwrap();
        fs::write(root.join("one/index.html"), "").unwrap();
        fs::write(root.join("two/index.html"), "").unwrap();
        assert!(find_game_entry(&root).unwrap_err().contains("Multiple games"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn legacy_records_default_to_website_view() {
        let old = r#"{"id":"abc","name":"Legacy","kind":"online","url":"https://example.com","port":null}"#;
        let parsed: InstalledApp = serde_json::from_str(old).unwrap();
        assert!(!parsed.game_focus);
        assert!(parsed.entry_path.is_none());
        assert_eq!(parsed.window_mode, WindowMode::Windowed);
        assert_eq!(parsed.window_width, 1150);
        assert_eq!(parsed.touch_buttons.len(), 7);
        assert_eq!(parsed.window_height, 760);
    }
}
