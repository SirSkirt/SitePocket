//! Experimental same-origin resource capture for static HTML5 games.
//! No DRM bypass, login-cookie extraction, private-network access or CDN copying.
//! Downloaded packages are *candidates*, never declared offline-verified.
use std::{collections::{HashSet, VecDeque}, fs, io::Read, path::{Path, PathBuf}, time::Duration};
use regex::Regex;
use reqwest::{blocking::Client, redirect::Policy};
use serde_json::json;
use url::Url;
use webcask_core::InstalledApp;

const MAX_FILES: usize = 2500;
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
const MAX_FILE: u64 = 128 * 1024 * 1024;

fn safe_origin(u: &Url) -> bool {
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() ||
       !u.username().is_empty() || u.password().is_some() { return false; }
    let host = u.host_str().unwrap_or("").to_lowercase();
    if host == "localhost" || host.ends_with(".localhost") ||
        host.ends_with(".local") || host.ends_with(".internal") || host.ends_with(".test") ||
        host.parse::<std::net::IpAddr>().is_ok() { return false; }
    true
}
fn same_origin(base: &Url, other: &Url) -> bool {
    safe_origin(other) && base.origin() == other.origin()
}
fn resource_path(url: &Url) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for segment in url.path_segments().ok_or("URL is not hierarchical")? {
        if segment.is_empty() || segment == "." { continue; }
        if segment == ".." || segment.chars().any(|ch| ch.is_control() ||
          matches!(ch, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')) {
            return Err("Unsafe URL path component".into());
        }
        out.push(segment);
    }
    if url.path().ends_with('/') || out.as_os_str().is_empty() {
        out.push("index.html");
    }
    Ok(out)
}
fn may_be_asset(u: &Url) -> bool {
    let p = u.path().to_ascii_lowercase();
    const EXTENSIONS: &[&str] = &[".js", ".mjs", ".css", ".json", ".wasm", ".data", ".bin",
      ".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".ico", ".mp3", ".wav", ".ogg",
      ".m4a", ".mp4", ".webm", ".woff", ".woff2", ".ttf", ".otf", ".glb", ".gltf",
      ".obj", ".pck", ".pack", ".txt", ".xml", ".atlas", ".fnt", ".map", ".webmanifest"];
    EXTENSIONS.iter().any(|ext| p.ends_with(ext))
}
fn referers_from_text(base: &Url, text: &str, is_html: bool) -> Vec<Url> {
    let mut found = Vec::new();
    // Collect linked HTML assets, CSS url(...), ESM modules, and literal fetch calls.
    let re = Regex::new(r#"(?i)(?:src|href|poster|data-src)\s*=\s*["']([^"'<>]+)["']|url\(\s*["']?([^"')\s]+)|(?:fetch|import)\s*\(\s*["']([^"']+)["']|(?:from)\s*["']([^"']+)["']"#).unwrap();
    for cap in re.captures_iter(text).take(7000) {
        let value = (1..=4).find_map(|i| cap.get(i).map(|m| m.as_str())).unwrap_or("");
        if value.starts_with("data:") || value.starts_with("blob:") || value.starts_with('#') { continue; }
        if let Ok(next) = base.join(value) {
            if may_be_asset(&next) || (is_html && value.ends_with(".html")) { found.push(next); }
        }
    }
    found
}
fn inject_base(source: &str, initial: &Url) -> String {
    let folder = initial.join("./").map(|u| u.path().to_owned()).unwrap_or("/".into());
    let tag = format!("<base href=\"{}\">", folder.replace('"', "%22"));
    let lower = source.to_ascii_lowercase();
    if let Some(pos) = lower.find("<head>") {
        let idx = pos + "<head>".len();
        format!("{}{}{}", &source[..idx], tag, &source[idx..])
    } else {
        format!("{tag}{source}")
    }
}
fn rewrite_absolute(text: &str, original: &Url) -> String {
    let origin = original.origin().ascii_serialization();
    text.replace(&origin, "")
        .replace(&format!("//{}", original.host_str().unwrap_or("")), "")
}

pub fn download_game(source: InstalledApp, resources: HashSet<String>) -> Result<String, String> {
    let url = Url::parse(source.url.as_deref().ok_or("This is not an online app")?)
        .map_err(|e| e.to_string())?;
    if !safe_origin(&url) { return Err("Capture requires a public HTTP(S) domain".into()); }
    let staging = webcask_core::root()?.join("capture-staging").join(&source.id);
    // This directory is managed solely by the app, never supplied by web content.
    if staging.exists() { fs::remove_dir_all(&staging).map_err(|e| e.to_string())?; }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let outcome = capture_to(&source, &url, resources, &staging);
    let _ = fs::remove_dir_all(&staging);
    outcome
}
fn capture_to(source: &InstalledApp, page: &Url, resources: HashSet<String>, stage: &Path) -> Result<String, String> {
    let client = Client::builder().redirect(Policy::none())
        .timeout(Duration::from_secs(18))
        .user_agent("Mozilla/5.0 (compatible; SitePocketCapture/0.1)")
        .build().map_err(|e| e.to_string())?;
    let mut queue = VecDeque::<(Url, usize)>::new();
    queue.push_back((page.clone(), 0));
    for resource in resources {
        if let Ok(link) = Url::parse(&resource) {
            if same_origin(page, &link) { queue.push_back((link, 0)); }
        }
    }
    let mut visited = HashSet::new();
    let mut saved_paths = HashSet::new();
    let mut failures = Vec::<String>::new();
    let mut saved = 0_usize;
    let mut bytes_total = 0_u64;
    let mut initial_html = None;
    while let Some((link, depth)) = queue.pop_front() {
        if visited.len() >= MAX_FILES { failures.push("File limit reached".into()); break; }
        if !same_origin(page, &link) { continue; }
        let key = link.as_str().to_owned();
        if !visited.insert(key) { continue; }
        let relative = match resource_path(&link) {
            Ok(p) => p,
            Err(msg) => { failures.push(msg); continue; }
        };
        if !saved_paths.insert(relative.clone()) { continue; } // Query-string collision.
        let response = match client.get(link.clone()).send() {
            Ok(r) => r, Err(e) => { failures.push(format!("{}: {e}", link.path())); continue; }
        };
        if !response.status().is_success() {
            failures.push(format!("{}: HTTP {}", link.path(), response.status()));
            continue;
        }
        if response.content_length().is_some_and(|size| size > MAX_FILE) {
            failures.push(format!("{}: exceeds per-file limit", link.path()));
            continue;
        }
        let mime = response.headers().get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
        let mut buffer = Vec::new();
        match response.take(MAX_FILE + 1).read_to_end(&mut buffer) {
            Ok(_) => {}, Err(e) => { failures.push(format!("{}: {e}", link.path())); continue; }
        }
        if buffer.len() as u64 > MAX_FILE { failures.push(format!("{}: too large", link.path())); continue; }
        if bytes_total + buffer.len() as u64 > MAX_TOTAL {
            failures.push("Total download limit reached (512 MiB)".into());
            break;
        }
        if link == *page {
            if !mime.contains("html") && !String::from_utf8_lossy(&buffer).trim_start().starts_with('<') {
                return Err("Website did not return an HTML game page; offline capture unavailable".into());
            }
            let html = String::from_utf8_lossy(&buffer).to_string();
            initial_html = Some(html.clone());
            // The captured page can live at /games/foo instead of /index.html.
            let mut text = inject_base(&html, page);
            text = rewrite_absolute(&text, page);
            fs::write(stage.join("index.html"), text).map_err(|e| e.to_string())?;
        } else {
            let dest = stage.join(&relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            if let Ok(text) = std::str::from_utf8(&buffer) {
                if mime.contains("javascript") || mime.contains("css") || mime.contains("html")
                    || relative.extension().is_some_and(|s| s == "js" || s == "mjs" || s == "css") {
                    buffer = rewrite_absolute(text, page).into_bytes();
                }
            }
            fs::write(dest, &buffer).map_err(|e| e.to_string())?;
        }
        saved += 1;
        bytes_total += buffer.len() as u64;
        if depth < 3 && (mime.contains("html") || mime.contains("javascript") ||
            mime.contains("css") || relative.extension().is_some_and(|s| s == "js" || s == "css")) {
            let text = if link == *page { initial_html.as_deref().unwrap_or("") }
                else { std::str::from_utf8(&buffer).unwrap_or("") };
            for found in referers_from_text(&link, text, mime.contains("html")) {
                if same_origin(page, &found) { queue.push_back((found, depth + 1)); }
            }
        }
    }
    if initial_html.is_none() {
        return Err("Couldn't capture the game's HTML entry point. It may need authentication or block downloads.".into());
    }
    if saved <= 1 {
        return Err("No game resources could be captured; no offline app was installed.".into());
    }
    let report = json!({
        "status": "unverified",
        "source": page.as_str(),
        "capturedFiles": saved,
        "capturedBytes": bytes_total,
        "failedRequests": failures,
        "limits": {"maxFiles": MAX_FILES, "maxBytes": MAX_TOTAL},
        "note": "Not proof of offline completeness. Turn off your network and test the copy."
    });
    fs::write(stage.join("webcask-capture-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let imported = webcask_core::import_game(&format!("{} (Offline)", source.name), stage)?;
    if source.game_focus { let _ = webcask_core::set_game_focus(&imported.id, true); }
    let warning = if report["failedRequests"].as_array().is_some_and(|a| !a.is_empty()) {
        format!(" Some files failed ({}); check the capture report.", failures.len())
    } else { String::new() };
    Ok(format!("Saved offline candidate '{}' ({} files, {:.1} MiB).{} Disconnect your internet and test it in SitePocket.",
        imported.name, saved, bytes_total as f64 / (1024.0 * 1024.0), warning))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_public_sites() {
        assert!(!safe_origin(&Url::parse("http://127.0.0.1:8080/").unwrap()));
        assert!(!safe_origin(&Url::parse("http://localhost/").unwrap()));
        assert!(safe_origin(&Url::parse("https://example.com/game/").unwrap()));
    }
    #[test] fn asset_path_rules() {
        let u=Url::parse("https://example.com/game/packs/app.wasm?v=2").unwrap();
        assert_eq!(resource_path(&u).unwrap(), PathBuf::from("game/packs/app.wasm"));
        assert!(same_origin(&u, &Url::parse("https://example.com/game/a.js").unwrap()));
        assert!(!same_origin(&u, &Url::parse("https://cdn.example.com/a.js").unwrap()));
    }
}
