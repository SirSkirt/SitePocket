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
        url: Some(parsed.to_string()), port: None };
    let mut apps = list_apps()?;
    apps.push(app.clone());
    save_apps(&apps)?;
    Ok(app)
}
pub fn import_game(name: &str, source: &Path) -> Result<InstalledApp, String> {
    let name = checked_name(name)?;
    let id = Uuid::new_v4().to_string();
    let target = app_path(&id)?.join("site");
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    if let Err(e) = import_files(source, &target) {
        let _ = fs::remove_dir_all(app_path(&id)?);
        return Err(e);
    }
    if !target.join("index.html").is_file() {
        let _ = fs::remove_dir_all(app_path(&id)?);
        return Err("Folder or ZIP must contain index.html at its root".into());
    }
    let mut apps = list_apps()?;
    let taken: Vec<u16> = apps.iter().filter_map(|a| a.port).collect();
    let port = (41000..61000).find(|p| !taken.contains(p)).ok_or("No local port available")?;
    let app = InstalledApp { id, name, kind: AppKind::Local, url: None, port: Some(port) };
    apps.push(app.clone());
    if let Err(e) = save_apps(&apps) {
        let _ = fs::remove_dir_all(app_path(&app.id)?);
        return Err(e);
    }
    Ok(app)
}
fn import_files(source: &Path, target: &Path) -> Result<(), String> {
    const LIMIT: u64 = 2 * 1024 * 1024 * 1024;
    const FILE_LIMIT: usize = 20_000;
    let mut total = 0_u64;
    if source.is_dir() {
        let root = source.canonicalize().map_err(|e| e.to_string())?;
        for (n, entry) in walkdir::WalkDir::new(&root).follow_links(false).into_iter().enumerate() {
            if n > FILE_LIMIT { return Err("Too many game files".into()); }
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().is_symlink() { return Err("Symlinks are not supported".into()); }
            let relative = entry.path().strip_prefix(&root).map_err(|e| e.to_string())?;
            let dest = target.join(relative);
            if entry.file_type().is_dir() { fs::create_dir_all(&dest).map_err(|e| e.to_string())?; continue; }
            if !entry.file_type().is_file() { continue; }
            total = total.checked_add(entry.metadata().map_err(|e| e.to_string())?.len()).ok_or("Game too large")?;
            if total > LIMIT { return Err("Game exceeds the 2 GiB import limit".into()); }
            if let Some(parent) = dest.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            fs::copy(entry.path(), dest).map_err(|e| e.to_string())?;
        }
    } else if source.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let file = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        if archive.len() > FILE_LIMIT { return Err("Archive contains too many entries".into()); }
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let relative = entry.enclosed_name().ok_or("Archive contains an unsafe file path")?.to_owned();
            let dest = target.join(relative);
            if entry.is_dir() { fs::create_dir_all(&dest).map_err(|e| e.to_string())?; continue; }
            total = total.checked_add(entry.size()).ok_or("Game too large")?;
            if total > LIMIT { return Err("Game exceeds the 2 GiB import limit".into()); }
            if let Some(parent) = dest.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
            let mut out = fs::File::create(dest).map_err(|e| e.to_string())?;
            io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    } else { return Err("Select an HTML5 game folder or .zip file".into()); }
    Ok(())
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
        let app = InstalledApp { id: Uuid::new_v4().to_string(), name: "Test".into(), kind: AppKind::Local, url: None, port: Some(42000) };
        let value = serde_json::to_string(&app).unwrap();
        let back: InstalledApp = serde_json::from_str(&value).unwrap();
        assert!(matches!(back.kind, AppKind::Local));
    }
}
