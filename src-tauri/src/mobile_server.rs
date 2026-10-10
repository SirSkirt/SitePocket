//! Read-only loopback asset server for ZIP-imported HTML5 games on Android.
//! Serves the actual index.html directory as the URL root, preserving
//! absolute (/assets/a.js) and relative (assets/a.js) asset references.
//! No internet is required after importing a self-contained game.
use std::{
    collections::HashMap, fs,
    io::{self, BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::Duration,
};
use webcask_core::{AppKind, InstalledApp};

static SERVERS: OnceLock<Mutex<HashMap<String, u16>>> = OnceLock::new();

pub fn serve(app: &InstalledApp) -> Result<String, String> {
    if !matches!(app.kind, AppKind::Local) {
        return Err("Only imported games use the local game server.".into());
    }
    let site = webcask_core::app_path(&app.id)?.join("site")
        .canonicalize().map_err(|e| format!("Imported game files are missing: {e}"))?;
    let entry = Path::new(app.entry_path.as_deref().unwrap_or("index.html"));
    if entry.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err("Imported game's entry point contains an unsafe path.".into());
    }
    let entry_absolute = site.join(entry)
        .canonicalize().map_err(|e| format!("Can't find this game's index.html: {e}"))?;
    if !entry_absolute.starts_with(&site) || !entry_absolute.is_file() {
        return Err("The game's HTML entry point is missing. Try importing it again.".into());
    }
    let root = entry_absolute.parent()
        .ok_or("The game has no parent directory")?.to_path_buf();
    let entry_filename = entry_absolute.file_name()
        .ok_or("Game entry filename is missing")?.to_os_string();
    let port = app.port.ok_or("This game has no assigned local port.")?;
    let registry = SERVERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = registry.lock().map_err(|_| "Local game server lock is unavailable")?;
    if guard.get(&app.id).copied() == Some(port) {
        return Ok(format!("http://127.0.0.1:{port}/"));
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
        .map_err(|e| format!("Local game port {port} is unavailable: {e}"))?;
    guard.insert(app.id.clone(), port);
    let game_id = app.id.clone();
    crate::diagnostics::record(&app.id, "info", "runtime", "server-started",
        "Local game server listening", &format!("127.0.0.1:{port}"));
    thread::spawn(move || {
        for incoming in listener.incoming() {
            match incoming {
                Ok(stream) => {
                    let root = root.clone();
                    let index = entry_filename.clone();
                    let game_id = game_id.clone();
                    thread::spawn(move || {
                        if let Err(err) = handle_request(stream, &root, &index, port, &game_id) {
                            crate::diagnostics::record(&game_id, "error", "assets",
                                "server-request-failed", "Local game server request failed",
                                &err.to_string());
                            eprintln!("WebCask Android asset request: {err}");
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });
    Ok(format!("http://127.0.0.1:{port}/"))
}

fn handle_request(mut stream: TcpStream, root: &Path, index: &std::ffi::OsStr,
    port: u16, game_id: &str) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut request_line = String::new();
    let mut input = BufReader::new(stream.try_clone()?);
    if input.read_line(&mut request_line)? > 4096 {
        return response(&mut stream, 400, "text/plain", b"Invalid request", false);
    }
    let mut words = request_line.split_whitespace();
    let method = words.next().unwrap_or("");
    let target = words.next().unwrap_or("");
    if method != "GET" && method != "HEAD" && method != "POST" {
        return response(&mut stream, 405, "text/plain", b"Method not allowed", false);
    }
    let head = method == "HEAD";
    let expected = format!("127.0.0.1:{port}");
    let mut valid_host = false;
    let mut origin: Option<String> = None;
    let mut content_length = 0_usize;
    for _ in 0..120 {
        let mut header = String::new();
        let n = input.read_line(&mut header)?;
        if n == 0 || header == "\r\n" || header == "\n" { break; }
        if header.len() > 8192 { return response(&mut stream, 400, "text/plain", b"Invalid header", head); }
        if let Some((name, value)) = header.trim_end().split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                valid_host = value.trim().eq_ignore_ascii_case(&expected);
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.trim().to_owned());
            } else if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse::<usize>().unwrap_or(usize::MAX);
            }
        }
    }
    if !valid_host {
        return response(&mut stream, 403, "text/plain", b"Host not allowed", head);
    }
    let url_path = target.split(['?', '#']).next().unwrap_or("/");
    if method == "POST" {
        if url_path != "/__webcask_debug" ||
           origin.as_deref() != Some(format!("http://{expected}").as_str()) ||
           content_length == 0 || content_length > 4096 {
            return response(&mut stream, 403, "text/plain", b"Diagnostics not allowed", false);
        }
        let mut buffer = vec![0u8; content_length];
        input.read_exact(&mut buffer)?;
        let record: serde_json::Value = match serde_json::from_slice(&buffer) {
            Ok(value) => value,
            Err(_) => return response(&mut stream, 400, "text/plain", b"Invalid diagnostics", false),
        };
        let kind = record.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let message = record.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let detail = record.get("detail").and_then(|v| v.as_str()).unwrap_or("");
        match kind {
            "page-ready" => crate::diagnostics::ready(game_id, "page-ready"),
            "script-error" | "promise-error" => crate::diagnostics::record(game_id,
                "error", "game", kind, message, detail),
            // "Script error." with no source is a browser security-redacted
            // exception, NOT evidence that the game's own code caused it.
            "opaque-script-error" => crate::diagnostics::record(game_id,
                "error", "unattributed", kind, message, detail),
            "resource-error" => crate::diagnostics::record(game_id,
                "warning", "assets", kind, message, detail),
            "long-task" => crate::diagnostics::record(game_id,
                "warning", "game", kind, message, detail),
            _ => {}
        }
        return response(&mut stream, 204, "text/plain", b"", false);
    }
    let path = match percent_encoding::percent_decode_str(url_path).decode_utf8() {
        Ok(p) => p,
        Err(_) => return response(&mut stream, 400, "text/plain", b"Invalid URL", head),
    };
    let relative = path.trim_start_matches('/');
    if relative.contains('\\') ||
        Path::new(relative).components().any(|c| !matches!(c, Component::Normal(_))) {
        return response(&mut stream, 403, "text/plain", b"Unsafe path", head);
    }
    let selected = if relative.is_empty() { root.join(index) } else { root.join(relative) };
    let actual = match selected.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            let lower = relative.to_ascii_lowercase();
            if [".js", ".mjs", ".wasm", ".json", ".bin", ".data", ".css"]
                .iter().any(|ext| lower.ends_with(ext)) {
                crate::diagnostics::record(game_id, "error", "assets", "missing-file",
                    "A game asset was not found", relative);
            }
            return response(&mut stream, 404, "text/plain", b"Not Found", head);
        },
    };
    if !actual.starts_with(root) || !actual.is_file() {
        return response(&mut stream, 403, "text/plain", b"Outside game folder", head);
    }
    let mut file = fs::File::open(&actual)?;
    let length = file.metadata()?.len();
    let mime = if actual.extension().is_some_and(|e| e.eq_ignore_ascii_case("wasm")) {
        "application/wasm".to_owned()
    } else {
        mime_guess::from_path(&actual).first_or_octet_stream().to_string()
    };
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {length}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Cross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\n\
         Cross-Origin-Resource-Policy: same-origin\r\n\
         Cache-Control: no-cache\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes())?;
    if !head { io::copy(&mut file, &mut stream)?; }
    Ok(())
}

fn response(stream: &mut TcpStream, code: u16, mime: &str, bytes: &[u8], head: bool)
    -> io::Result<()> {
    let label = match code {
        400 => "Bad Request", 403 => "Forbidden", 404 => "Not Found",
        405 => "Method Not Allowed", 204 => "No Content", _ => "Error",
    };
    write!(stream,
        "HTTP/1.1 {code} {label}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n", bytes.len())?;
    if !head { stream.write_all(bytes)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serves_nested_game_assets_and_prevents_escape() {
        let sandbox = std::env::temp_dir().join(format!("webcask-local-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(sandbox.join("Build/assets")).unwrap();
        fs::write(sandbox.join("Build/index.html"), b"zip-game").unwrap();
        fs::write(sandbox.join("Build/assets/game.wasm"), b"wasm bytes").unwrap();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let root = sandbox.join("Build");
        let file = std::ffi::OsStr::new("index.html");
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (stream, _) = listener.accept().unwrap();
                handle_request(stream, &root, file, port, "fixture-test").unwrap();
            }
        });
        fn hit(port:u16,path:&str) -> String {
            use std::io::Read;
            let mut socket = TcpStream::connect((Ipv4Addr::LOCALHOST,port)).unwrap();
            write!(socket, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
            let mut response = String::new();
            socket.read_to_string(&mut response).unwrap();
            response
        }
        assert!(hit(port, "/").ends_with("zip-game"));
        assert!(hit(port, "/assets/game.wasm").contains("Content-Type: application/wasm"));
        assert!(hit(port, "/%2e%2e/outside").contains("403 Forbidden"));
        server.join().unwrap();
        let _ = fs::remove_dir_all(sandbox);
    }
}
