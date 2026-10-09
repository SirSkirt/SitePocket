# SitePocket + WebCask Runtime

**SitePocket** is a Tauri/TypeScript/Rust web app installer and manager.
**WebCask Runtime** is an independent Rust/Wry/WebView2 application launcher.
A shortcut launches the selected web app through WebCask, without opening the
manager.

> v0.1.0 Alpha, Windows x64. Experimental; not yet verified on a Windows PC.

## Implemented in this version

- Install a website URL even when it doesn't have a PWA manifest.
- Import a local HTML5/JavaScript/WebAssembly game folder or ZIP, including
  exports with **one wrapper directory** (e.g. `MyGame/dist/index.html`).
  SitePocket finds a unique `index.html` up to six folders deep, preserves the
  source tree, and serves the containing directory as the web root so absolute
  asset paths such as `/assets/main.js` work.
- Import status and errors appear inline **to the left of Import Game**. If
  import fails, SitePocket keeps the selected source and name so you can fix
  the problem. Diagnostics distinguish invalid ZIPs, missing `index.html`,
  ambiguous multiple game entries, oversized archives, and copy errors.
- Launch individual apps via a standalone WebCask process and webview.
- Persist separate user-data/browser profiles for installed apps.
- Serve local games via a read-only 127.0.0.1 HTTP server with WASM MIME,
  path traversal restrictions and cross-origin isolation headers.
- Create Desktop shortcuts; uninstall apps and remove their local profiles.
- **Game Options** on every installed app: save a per-app **Launch in Game View**
  preference, and choose **Windowed**, **Maximized**, or **Fullscreen** at startup.
  Windowed mode has customizable initial width/height (480–3840 by 320–2160).
  WebCask reads these settings at launch even when starting from a desktop
  shortcut. Existing installs default to their previous 1150 × 760 window and
  preserve their previously configured Game View state.
  Borderless fullscreen covers a display without changing its resolution; use
  Alt+F4 to close when the webview has no title bar. Settings take effect the
  next time the app launches.
- **Game Focus Mode:** per-app preference expands the largest visible HTML5 canvas
  or embedded iframe into the window and hides surrounding site elements.
  Ctrl+Shift+G toggles page/game view; Ctrl+Shift+P lets you pick a game element
  manually. This changes presentation; it does not download or modify game files.
  Some websites or custom DOM layouts may not work with the heuristic.
- **Experimental website-to-offline capture:** on an online app choose "Capture offline",
  explore it while online to discover resource requests, then click "Save Offline
  Candidate" (or Ctrl+Shift+S). WebCask downloads observed same-origin resources,
  follows local HTML/CSS/JS references, and registers a separate local game entry.
  The manager's "Refresh library" button displays the result.
- Windows Actions build, upload an NSIS installer and create a GitHub prerelease.

**Limitations:** This experimental downloader does NOT guarantee playable offline
games: only HTTP(S) resources from the same origin are fetched; CDNs, server
authentication, cookie-dependent requests, streaming multiplayer, and dynamic
asset URL generation can prevent offline play. Captures use a separate browser
profile for the downloaded game, so existing online saves may not carry over.
No bypass of DRM, paid access, login protection or usage permissions is attempted.
The report in each downloaded game's directory records missing resources and
marks offline status "unverified"; **test by disconnecting your network**.
Imports require a game with local assets. When a ZIP contains multiple
`index.html` files and no root index, select a more specific folder rather
than letting the importer guess. Files outside the selected game entry's
directory are not served for security and compatibility.
Network-only
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
