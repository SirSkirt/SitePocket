//! Per-app icons for library, WebCask windows, and Windows shortcuts.
//! Website discovery is best effort; only validated public HTTPS/HTTP hosts
//! are contacted by Rust. No redirects, cookies, or Tauri native IPC to sites.
use std::{fs, io::{Cursor, Read}, net::{IpAddr, SocketAddr, ToSocketAddrs}, path::{Path, PathBuf}, time::{Duration, Instant}};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{imageops::FilterType, DynamicImage, ImageReader};
use image::ImageEncoder;
use reqwest::{blocking::Client, redirect::Policy};
use scraper::{Html, Selector};
use url::Url;
use webcask_core::{AppKind, InstalledApp};

const MAX_ICON_BYTES: usize = 3 * 1024 * 1024;
const MAX_HTML_BYTES: usize = 384 * 1024;
const MAX_DIM: u32 = 4096;

pub fn icon_path(id: &str) -> Result<PathBuf, String> {
    Ok(webcask_core::app_path(id)?.join("icon.ico"))
}
fn png_path(id: &str) -> Result<PathBuf, String> {
    Ok(webcask_core::app_path(id)?.join("icon.png"))
}
pub fn icon_data(id: &str) -> Option<String> {
    let data = fs::read(png_path(id).ok()?).ok()?;
    if data.len() > 300_000 { return None; }
    Some(format!("data:image/png;base64,{}", STANDARD.encode(data)))
}

fn icon_image(bytes: &[u8]) -> Result<DynamicImage, String> {
    if bytes.is_empty() || bytes.len() > MAX_ICON_BYTES {
        return Err("Icon image is empty or larger than 3 MiB.".into());
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format().map_err(|e| format!("Unrecognized image file: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIM);
    limits.max_image_height = Some(MAX_DIM);
    limits.max_alloc = Some(96 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| format!("Unsupported or damaged icon image: {e}"))
}
fn store_icon(id: &str, bytes: &[u8]) -> Result<String, String> {
    let picture = icon_image(bytes)?;
    let root = webcask_core::app_path(id)?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    // Keep aspect ratio and center on a transparent square.
    let mut canvas = image::RgbaImage::new(256, 256);
    let resized = picture.resize(256, 256, FilterType::Lanczos3).to_rgba8();
    let x = (256 - resized.width()) / 2;
    let y = (256 - resized.height()) / 2;
    image::imageops::overlay(&mut canvas, &resized, x.into(), y.into());

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(canvas.as_raw(), 256, 256, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("Could not encode PNG: {e}"))?;

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 256] {
        let rgba = image::imageops::resize(&canvas, size, size, FilterType::Lanczos3).into_raw();
        let entry = ico::IconDirEntry::encode(&ico::IconImage::from_rgba_data(size, size, rgba))
            .map_err(|e| format!("Could not encode Windows icon: {e}"))?;
        dir.add_entry(entry);
    }
    let mut ico_bytes = Vec::new();
    dir.write(&mut ico_bytes).map_err(|e| format!("Could not write Windows icon: {e}"))?;
    let png_file = root.join("icon.png");
    let ico_file = root.join("icon.ico");
    // Save the preview and shortcut icon at stable paths. Existing desktop
    // shortcuts need no new target after replacement.
    fs::write(&png_file, &png).map_err(|e| format!("Cannot save app icon PNG: {e}"))?;
    fs::write(&ico_file, ico_bytes).map_err(|e| format!("Cannot save app icon ICO: {e}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(png)))
}
pub fn set_custom_icon(id: &str, file: &Path) -> Result<String, String> {
    webcask_core::get_app(id)?;
    if !file.is_file() { return Err("Select an image file to use as the app icon.".into()); }
    let metadata = fs::metadata(file).map_err(|e| format!("Cannot read chosen icon: {e}"))?;
    if metadata.len() > MAX_ICON_BYTES as u64 {
        return Err("The selected icon is larger than 3 MiB. Choose a smaller PNG, JPG, WebP, or ICO file.".into());
    }
    let bytes = fs::read(file).map_err(|e| format!("Cannot open chosen icon: {e}"))?;
    store_icon(id, &bytes)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            if o[0] == 0 || o[0] == 10 || o[0] == 127 || o[0] >= 224 ||
                (o[0] == 100 && (64..=127).contains(&o[1])) ||
                (o[0] == 169 && o[1] == 254) ||
                (o[0] == 172 && (16..=31).contains(&o[1])) ||
                (o[0] == 192 && (o[1] == 0 || o[1] == 168)) ||
                (o[0] == 198 && (o[1] == 18 || o[1] == 19)) ||
                (o[0] == 192 && o[1] == 0 && o[2] == 2) ||
                (o[0] == 198 && o[1] == 51 && o[2] == 100) ||
                (o[0] == 203 && o[1] == 0 && o[2] == 113) {
                return false;
            }
            true
        },
        IpAddr::V6(v) => {
            let first = v.octets()[0];
            (0x20..=0x3f).contains(&first) && !v.is_loopback()
                && !v.is_unspecified() && v.to_ipv4_mapped().is_none()
        },
    }
}
fn valid_host(url: &Url) -> Result<(String, u16, SocketAddr), String> {
    if !matches!(url.scheme(), "https" | "http") ||
        !url.username().is_empty() || url.password().is_some() {
        return Err("Icon discovery supports public HTTP(S) websites only.".into());
    }
    let host = url.domain().ok_or("Icon discovery requires a public domain name.")?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".local") ||
        host.ends_with(".internal") || host.ends_with(".test") ||
        host.ends_with(".localhost") {
        return Err("Private/internal sites are not fetched for icons.".into());
    }
    let port = url.port_or_known_default().ok_or("Unknown icon URL port")?;
    if (url.scheme() == "https" && port != 443) || (url.scheme() == "http" && port != 80) {
        return Err("Automatic icon discovery supports standard HTTPS/HTTP ports.".into());
    }
    let addresses: Vec<SocketAddr> = (host, port).to_socket_addrs()
        .map_err(|e| format!("Cannot resolve website domain: {e}"))?.collect();
    if addresses.is_empty() || addresses.iter().any(|s| !public_ip(s.ip())) {
        return Err("The website resolves to an internal or unsupported network address.".into());
    }
    Ok((host.to_owned(), port, addresses[0]))
}
fn website_client(url: &Url) -> Result<Client, String> {
    let (host, _, address) = valid_host(url)?;
    Client::builder()
        .redirect(Policy::none()).no_proxy().resolve(&host, address)
        .timeout(Duration::from_secs(7))
        .user_agent("SitePocket/0.1 IconDiscovery")
        .build().map_err(|e| format!("Cannot start icon discovery: {e}"))
}
fn get_bytes(client: &Client, url: &Url, max: usize) -> Result<Vec<u8>, String> {
    let response = client.get(url.as_str()).send()
        .map_err(|e| format!("Icon request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("Website returned HTTP {}", response.status()));
    }
    if response.content_length().is_some_and(|len| len > max as u64) {
        return Err("Icon resource is too large.".into());
    }
    let mut output = Vec::new();
    response.take((max + 1) as u64).read_to_end(&mut output)
        .map_err(|e| format!("Could not download icon resource: {e}"))?;
    if output.len() > max { return Err("Icon resource exceeds size limit.".into()); }
    Ok(output)
}
fn add_link(base: &Url, value: &str, output: &mut Vec<Url>) {
    if let Ok(u) = base.join(value.trim()) {
        if u.origin() == base.origin() && matches!(u.scheme(), "http" | "https") &&
            !output.iter().any(|item| item == &u) {
            output.push(u);
        }
    }
}
fn website_candidates(base: &Url, html: &str, manifest: Option<&str>) -> Vec<Url> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("link[href]").unwrap();
    let mut candidates = Vec::new();
    if let Some(manifest_json) = manifest {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(manifest_json) {
            if let Some(arr) = val.get("icons").and_then(|v| v.as_array()) {
                for obj in arr.iter().take(24) {
                    if let Some(src) = obj.get("src").and_then(|x| x.as_str()) {
                        add_link(base, src, &mut candidates);
                    }
                }
            }
        }
    }
    for link in doc.select(&sel) {
        let rel = link.value().attr("rel").unwrap_or("").to_ascii_lowercase();
        if rel.split_whitespace().any(|r| r == "icon" || r == "apple-touch-icon" || r == "apple-touch-icon-precomposed") {
            if let Some(href) = link.value().attr("href") { add_link(base, href, &mut candidates); }
        }
    }
    if let Ok(url) = base.join("/favicon.ico") { add_link(base, url.as_str(), &mut candidates); }
    if let Ok(url) = base.join("/apple-touch-icon.png") { add_link(base, url.as_str(), &mut candidates); }
    candidates.truncate(16);
    candidates
}
fn manifest_href(base: &Url, html: &str) -> Option<Url> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("link[href]").ok()?;
    for link in doc.select(&sel) {
        let rel = link.value().attr("rel").unwrap_or("").to_ascii_lowercase();
        if rel.split_whitespace().any(|r| r == "manifest") {
            let link = base.join(link.value().attr("href")?).ok()?;
            if link.origin() == base.origin() { return Some(link); }
        }
    }
    None
}
pub fn fetch_website_icon(app: &InstalledApp) -> Result<String, String> {
    if !matches!(&app.kind, AppKind::Online) {
        return Err("Website icon refresh is only available for online apps.".into());
    }
    let origin = Url::parse(app.url.as_deref().ok_or("App has no website URL")?)
        .map_err(|e| format!("Invalid website URL: {e}"))?;
    let client = website_client(&origin)?;
    let html_bytes = get_bytes(&client, &origin, MAX_HTML_BYTES)?;
    let html = String::from_utf8_lossy(&html_bytes);
    let manifest = manifest_href(&origin, &html)
        .and_then(|url| get_bytes(&client, &url, MAX_HTML_BYTES).ok()
            .map(|data| (url, String::from_utf8_lossy(&data).to_string())));
    let mut choices = Vec::new();
    if let Some((manifest_url, json)) = &manifest {
        choices.extend(website_candidates(manifest_url, "", Some(json)));
    }
    choices.extend(website_candidates(&origin, &html, None));
    let mut failed = 0;
    let started = Instant::now();
    for link in choices.into_iter().take(10) {
        if started.elapsed() > Duration::from_secs(20) { break; }
        // No cross-origin downloads or redirects; unlike a browser the
        // manager should not forward user credentials to discovered links.
        let Ok(bytes) = get_bytes(&client, &link, MAX_ICON_BYTES) else { failed += 1; continue; };
        if let Ok(icon) = store_icon(&app.id, &bytes) { return Ok(icon); }
        failed += 1;
    }
    Err(format!("No compatible website icon found ({} resources checked). Choose a custom image instead.", failed))
}
pub fn detect_local_icon(app: &InstalledApp) -> Result<String, String> {
    if !matches!(&app.kind, AppKind::Local) { return Err("This is not an imported game.".into()); }
    let base = webcask_core::app_path(&app.id)?.join("site");
    let entry = base.join(app.entry_path.as_deref().unwrap_or("index.html"));
    let root = entry.parent().ok_or("Game entry path unavailable")?;
    let html = fs::read_to_string(&entry).unwrap_or_default();
    let mut candidates = Vec::<PathBuf>::new();
    let doc = Html::parse_document(&html);
    let sel = Selector::parse("link[href]").unwrap();
    for link in doc.select(&sel) {
        let rel = link.value().attr("rel").unwrap_or("").to_ascii_lowercase();
        if rel.split_whitespace().any(|r| r == "icon" || r == "apple-touch-icon") {
            if let Some(href) = link.value().attr("href") {
                let path = href.split(['?', '#']).next().unwrap_or("");
                let p = if path.starts_with('/') { root.join(path.trim_start_matches('/')) } else { root.join(path) };
                candidates.push(p);
            }
        }
    }
    for file in ["favicon.ico", "favicon.png", "apple-touch-icon.png", "icon.png", "icon.ico", "logo.png"] {
        candidates.push(root.join(file));
    }
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    for candidate in candidates.into_iter().take(30) {
        let Ok(real) = candidate.canonicalize() else { continue; };
        if !real.starts_with(&canonical_root) || !real.is_file() { continue; }
        if fs::metadata(&real).is_ok_and(|m| m.len() > MAX_ICON_BYTES as u64) { continue; }
        if let Ok(data) = fs::read(&real) {
            if let Ok(preview) = store_icon(&app.id, &data) { return Ok(preview); }
        }
    }
    Err("No bundled game icon found. Choose an image manually in Game Options.".into())
}
pub fn restore_detected_icon(id: &str) -> Result<String, String> {
    let app = webcask_core::get_app(id)?;
    match app.kind {
        AppKind::Online => fetch_website_icon(&app),
        AppKind::Local => detect_local_icon(&app),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_internal_hosts() {
        let url = Url::parse("http://localhost/").unwrap();
        assert!(valid_host(&url).is_err());
        let url = Url::parse("http://127.0.0.1/").unwrap();
        assert!(valid_host(&url).is_err());
        assert!(!public_ip("192.168.1.4".parse().unwrap()));
        assert!(!public_ip("10.0.0.2".parse().unwrap()));
        assert!(!public_ip("169.254.169.254".parse().unwrap()));
    }
    #[test]
    fn discovers_icons_and_same_origin_only() {
        let base = Url::parse("https://example.com/game").unwrap();
        let html = r#"<link rel="icon" href="/assets/pic.png"><link rel="icon" href="https://evil.example/icon.png">"#;
        let found = website_candidates(&base, html, None);
        assert!(found.iter().any(|u| u.as_str() == "https://example.com/assets/pic.png"));
        assert!(!found.iter().any(|u| u.host_str() == Some("evil.example")));
    }
    #[test]
    fn rejects_corrupt_image() {
        assert!(icon_image(b"not actually an image").is_err());
    }
}
