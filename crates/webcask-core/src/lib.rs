//! Application registry and game importer; no Tauri or browser privileges.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::{Path, PathBuf}};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppKind { Online, Local }

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
}
pub fn root() -> Result<PathBuf, String> {
    let path = dirs::data_local_dir().ok_or("Local application data directory unavailable")?.join("SitePocket");
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
        url: Some(parsed.to_string()), port: None, game_focus: false, entry_path: None };
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
    fn modes_roundtrip() {
        let app = InstalledApp { id: Uuid::new_v4().to_string(), name: "Test".into(), kind: AppKind::Local, url: None, port: Some(42000), game_focus: true, entry_path: Some("Build/index.html".into()) };
        let value = serde_json::to_string(&app).unwrap();
        let back: InstalledApp = serde_json::from_str(&value).unwrap();
        assert!(matches!(back.kind, AppKind::Local));
        assert!(back.game_focus);
        assert_eq!(back.entry_path.as_deref(), Some("Build/index.html"));
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
    }
}
