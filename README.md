# SitePocket + WebCask Runtime

**SitePocket** is a Tauri/TypeScript/Rust web app installer and manager.
**WebCask Runtime** is an independent Rust/Wry/WebView2 application launcher.
A shortcut launches the selected web app through WebCask, without opening the
manager.

> v0.1.0 Alpha, Windows x64. Experimental; not yet verified on a Windows PC.

## Implemented in this version

- Install a website URL even when it doesn't have a PWA manifest.
- Import a local HTML5/JavaScript/WebAssembly game directory or ZIP that has
  an index.html at its root.
- Launch individual apps via a standalone WebCask process and webview.
- Persist separate user-data/browser profiles for installed apps.
- Serve local games via a read-only 127.0.0.1 HTTP server with WASM MIME,
  path traversal restrictions and cross-origin isolation headers.
- Create Desktop shortcuts; uninstall apps and remove their local profiles.
- **Game Focus Mode:** per-app preference expands the largest visible HTML5 canvas
  or embedded iframe into the window and hides surrounding site elements.
  Ctrl+Shift+G toggles page/game view; Ctrl+Shift+P lets you pick a game element
  manually. This changes presentation; it does not download or modify game files.
  Some websites or custom DOM layouts may not work with the heuristic.
- Windows Actions build, upload an NSIS installer and create a GitHub prerelease.

**Limitations:** Automatic network game capture and verified offline conversion
are NOT implemented yet. Import requires a game with local assets. Network-only
features will stay network-dependent. The localhost server uses a stable per-app
port (a port conflict prevents launch), does not stream large files or support
HTTP Range requests in this alpha. Shortcuts use a shared runtime icon and
per-app taskbar identity is not implemented yet.

Remote websites cannot access native Tauri commands; only the bundled manager
frontend has those permissions. User-provided game files are untrusted and should
be imported only from trusted sources.

## Build on Windows

Prerequisites: Node.js 22+, stable Rust/MSVC toolchain and Visual Studio
C++ build tools. WebView2 must be installed.

1. Run \`npm install\`.
2. Run \`cargo test -p webcask-core\`.
3. Run \`cargo build --release -p webcask-runtime\`.
4. Create the directory \`src-tauri/resources\` and copy the runtime exe into
   \`src-tauri/resources/webcask-runtime.exe\`.
5. Run \`npm run tauri build -- --bundles nsis\`.
6. Find the installer in \`target/release/bundle/nsis/\`.

For local development, build WebCask in debug with
\`cargo build -p webcask-runtime\` and run \`npm run tauri dev\`.

The workflow builds on every push to main, uploading its NSIS artifact and
publishing a numbered prerelease tag (v0.1.0-alpha.BUILD_NUMBER).

## Roadmap

Automatic request interception/game asset capture, offline verification,
gamepad support, HTTP range responses, per-app icons and Windows taskbar
AppUserModelIDs, richer profiles, native permission UI.

## License

MIT. See LICENSE.
