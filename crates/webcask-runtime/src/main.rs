#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod capture;
use std::{collections::HashSet, fs, io::{Read, Write}, net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Component, Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
    thread, time::Duration};
use webcask_core::{AppKind, InstalledApp};
use winit::{application::ApplicationHandler, dpi::LogicalSize, event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy}, window::{Window, WindowId}};
use wry::{WebContext, WebView, WebViewBuilder};

enum CaptureEvent { Finished(Result<String, String>) }

struct CaptureSession {
    source: InstalledApp,
    resources: Arc<Mutex<HashSet<String>>>,
    saving: Arc<AtomicBool>,
    proxy: EventLoopProxy<CaptureEvent>,
}

struct WebCask {
    title: String,
    url: String,
    window: Option<Window>,
    webview: Option<WebView>,
    context: Option<WebContext>,
    profile: PathBuf,
    game_focus: bool,
    capture: Option<CaptureSession>,
}

impl ApplicationHandler<CaptureEvent> for WebCask {
    fn user_event(&mut self, _events: &ActiveEventLoop, event: CaptureEvent) {
        let CaptureEvent::Finished(result) = event;
        let message = match result {
            Ok(value) => value,
            Err(error) => format!("Capture failed: {error} (close and restart capture to try again)"),
        };
        if let Some(view) = &self.webview {
            if let Ok(payload) = serde_json::to_string(&message) {
                let _ = view.evaluate_script(&format!(
                    "window.__webcaskCaptureStatus && window.__webcaskCaptureStatus({payload})"
                ));
            }
        }
    }

    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let attrs = Window::default_attributes().with_title(&self.title)
            .with_inner_size(LogicalSize::new(1150., 760.))
            .with_min_inner_size(LogicalSize::new(480., 320.));
        let window = match events.create_window(attrs) {
            Ok(w) => w, Err(e) => { eprintln!("Window: {e}"); events.exit(); return; }
        };
        let mut context = WebContext::new(Some(self.profile.clone()));
        let mut builder = WebViewBuilder::new_with_web_context(&mut context).with_url(&self.url);
        if self.game_focus {
            builder = builder.with_initialization_script_for_main_only(
                include_str!("game_focus.js"), true
            );
        }
        if let Some(session) = &self.capture {
            // Only capture mode exposes this one-purpose IPC channel to website JS.
            // Messages cannot request paths, commands or privileged actions.
            let source = session.source.clone();
            let seen = session.resources.clone();
            let saving = session.saving.clone();
            let proxy = session.proxy.clone();
            builder = builder
                .with_initialization_script_for_main_only(include_str!("capture.js"), true)
                .with_ipc_handler(move |request| {
                    if request.body().len() > 100_000 { return; }
                    let Ok(message) = serde_json::from_str::<serde_json::Value>(request.body()) else { return; };
                    match message.get("kind").and_then(|v| v.as_str()) {
                        Some("resources") if !saving.load(Ordering::Relaxed) => {
                            if let Some(urls) = message.get("urls").and_then(|v| v.as_array()) {
                                if let Ok(mut collected) = seen.lock() {
                                    for candidate in urls.iter().take(3000) {
                                        if collected.len() >= 3500 { break; }
                                        if let Some(url) = candidate.as_str() {
                                            if url.len() <= 2048 { collected.insert(url.to_owned()); }
                                        }
                                    }
                                }
                            }
                        }
                        Some("finish") => {
                            if saving.swap(true, Ordering::SeqCst) { return; }
                            let urls = seen.lock().map(|g| g.clone()).unwrap_or_default();
                            let app = source.clone();
                            let sender = proxy.clone();
                            thread::spawn(move || {
                                let result = capture::download_game(app, urls);
                                let _ = sender.send_event(CaptureEvent::Finished(result));
                            });
                        }
                        _ => {}
                    }
                });
        }
        match builder.build(&window) {
            Ok(webview) => {
                self.webview = Some(webview);
                self.context = Some(context);
                self.window = Some(window);
            }
            Err(e) => { eprintln!("Webview: {e}"); events.exit(); }
        }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if self.game_focus {
                    if let Some(webview) = &self.webview {
                        let _ = webview.evaluate_script("window.__webcaskRescale && window.__webcaskRescale()");
                    }
                }
            }
            WindowEvent::CloseRequested | WindowEvent::Destroyed => events.exit(),
            _ => {}
        }
    }
}
fn main() {
    if let Err(err) = run() {
        eprintln!("WebCask Runtime: {err}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let id = args.windows(2).find(|pair| pair[0] == "--app-id")
        .map(|pair| pair[1].as_str()).ok_or("Usage: webcask-runtime --app-id UUID")?;
    let app = webcask_core::get_app(id)?;
    let profile = webcask_core::profile_path(id)?;
    fs::create_dir_all(&profile).map_err(|e| e.to_string())?;
    let game_focus = app.game_focus;
    let capture_mode = args.iter().any(|arg| arg == "--capture");
    if capture_mode && !matches!(app.kind, AppKind::Online) {
        return Err("Only online apps can be captured".into());
    }
    let url = match app.kind {
        AppKind::Online => app.url.clone().ok_or("Website URL missing")?,
        AppKind::Local => serve_local(&app)?,
    };
    let events = EventLoop::<CaptureEvent>::with_user_event().build().map_err(|e| e.to_string())?;
    let capture = if capture_mode {
        Some(CaptureSession {
            source: app.clone(),
            resources: Arc::new(Mutex::new(HashSet::new())),
            saving: Arc::new(AtomicBool::new(false)),
            proxy: events.create_proxy(),
        })
    } else { None };
    events.run_app(&mut WebCask {
        title: if capture_mode { format!("WebCask Capture · {}", app.name) } else { app.name },
        url, window: None, webview: None, context: None, profile, game_focus, capture
    }).map_err(|e| e.to_string())
}
fn serve_local(app: &InstalledApp) -> Result<String, String> {
    let root = webcask_core::app_path(&app.id)?.join("site")
        .canonicalize().map_err(|e| e.to_string())?;
    let port = app.port.ok_or("Missing game port")?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| format!("Port {port} unavailable: {e}"))?;
    thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => { let _ = handle_request(stream, &root, port); }
                Err(_) => break,
            }
        }
    });
    Ok(format!("http://127.0.0.1:{port}/"))
}
// Restricted read-only loopback asset server; no internet connection needed.
fn handle_request(mut stream: TcpStream, root: &Path, port: u16) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = [0_u8; 8192];
    let size = stream.read(&mut buf)?;
    let raw = String::from_utf8_lossy(&buf[..size]);
    let mut lines = raw.lines();
    let request = lines.next().unwrap_or("");
    let mut tokens = request.split_whitespace();
    let method = tokens.next().unwrap_or("");
    let target = tokens.next().unwrap_or("");
    if method != "GET" && method != "HEAD" {
        return reply(&mut stream, 405, "text/plain", b"Method Not Allowed", false);
    }
    let expected = format!("host: 127.0.0.1:{port}");
    if !lines.any(|line| line.trim().eq_ignore_ascii_case(&expected)) {
        return reply(&mut stream, 403, "text/plain", b"Forbidden", method == "HEAD");
    }
    let path_str = target.split('?').next().unwrap_or("/");
    let decoded = percent_encoding::percent_decode_str(path_str).decode_utf8();
    let Ok(decoded) = decoded else {
        return reply(&mut stream, 400, "text/plain", b"Invalid path", method == "HEAD");
    };
    let relative = decoded.trim_start_matches('/');
    let relative = if relative.is_empty() { "index.html" } else { relative };
    let path = Path::new(relative);
    if path.components().any(|p| !matches!(p, Component::Normal(_)))
        || relative.contains('\\') {
        return reply(&mut stream, 403, "text/plain", b"Forbidden", method == "HEAD");
    }
    let requested = root.join(path);
    let Ok(actual) = requested.canonicalize() else {
        return reply(&mut stream, 404, "text/plain", b"Not Found", method == "HEAD");
    };
    if !actual.starts_with(root) || !actual.is_file() {
        return reply(&mut stream, 403, "text/plain", b"Forbidden", method == "HEAD");
    }
    let content = fs::read(&actual)?;
    let mime = if actual.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("wasm")) {
        "application/wasm".into()
    } else { mime_guess::from_path(actual).first_or_octet_stream().to_string() };
    reply(&mut stream, 200, &mime, &content, method == "HEAD")
}
fn reply(stream: &mut TcpStream, code: u16, mime: &str, bytes: &[u8], head: bool) -> std::io::Result<()> {
    let reason = match code { 200=>"OK", 400=>"Bad Request", 403=>"Forbidden",
        404=>"Not Found", 405=>"Method Not Allowed", _=>"Error" };
    let headers = format!("HTTP/1.1 {code} {reason}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCross-Origin-Opener-Policy: same-origin\r\nCross-Origin-Embedder-Policy: require-corp\r\nCross-Origin-Resource-Policy: same-origin\r\nConnection: close\r\n\r\n", bytes.len());
    stream.write_all(headers.as_bytes())?;
    if !head { stream.write_all(bytes)?; }
    Ok(())
}
