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
- **Per-app icons:** SitePocket checks online sites for manifest icons, favicons,
  and Apple touch icons on install, and scans imported game folders for their
  bundled icons. A missing favicon never blocks installation.
- **Customize icons:** Open Game Options → Choose Image to use your own PNG,
  JPG, WebP, GIF, BMP, or ICO; Get Website Icon / Get Game Icon restores the
  discovered icon. Icons are stored locally as PNG (library preview) and a
  multi-size Windows ICO (16/32/48/256) for shortcuts, title bars, and taskbar.
  Existing desktop shortcuts are updated where possible; Windows icon cache
  or a pinned taskbar shortcut may require relaunching or re-pinning.
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
Automatic icon lookup is best effort. For safety it fetches only same-origin
public HTTP(S) resources, without redirects, proxy access, authentication,
or private-network requests; some websites use icons hosted on other domains,
and those apps may need a custom icon. The Tauri runtime never grants websites
native filesystem or icon-management privileges.
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


## Android preview — WebCask Touch

SitePocket now has an Android preview targeting Android 12L (API 32) or newer.
Android uses system WebView through a separate Tauri player window. The game
webview receives no SitePocket native commands.

From **Game Options**, open **Edit Touch Layout**. Drag buttons to position them,
choose labels, keyboard mappings and sizes, add and delete buttons (up to 24),
and toggle them per game. Controls support multi-touch holding and release.
The configuration persists across launches in the Rust registry.

**Compatibility:** Buttons emit synthetic browser KeyboardEvents. Games that
require trusted hardware keys or run in cross-origin embedded frames may ignore
them. Full native keyboard emulation is not yet implemented.

**Android offline ZIP games:** Open Offline Game in SitePocket, select
**Choose ZIP**, pick an HTML5/JavaScript/WASM ZIP using Android Files, and tap
**Import game**. The selected `content://` document is staged into private
app storage (the original ZIP is untouched), extracted with the same bounded
Rust importer as Windows, and appears in the library. Launch it through a
read-only `127.0.0.1` local asset server. It supports game-relative and
absolute asset paths, correct WASM content types, and runs without internet
provided every game resource is inside the ZIP. Imported games get the same
per-game editable touchscreen keyboard mappings.

**Android limitations:** The Android folder picker is unsupported; use ZIP.
Some games still need an online API, DRM check, external assets, or server,
and will not play fully offline. The local server does not yet implement
HTTP Range requests, which some media-heavy games require. Imported files
are not automatically transferred from Windows installs; import the ZIP on
Android. Browser-game capture and Windows-style desktop shortcuts are not
supported on Android. Android WebView compatibility varies with the installed
system provider. Android's Network Security Configuration enables cleartext
HTTP **only to localhost**, not arbitrary external sites.
Builds are debug-signed for sideload testing, not Google Play signing.

Build on Linux with JDK17, Android SDK/NDK, Rust Android target, Node22:

    npm install
    npx tauri icon src-tauri/icons/icon.png
    npx tauri android init --ci
    npx tauri android build --apk --debug --target aarch64

Windows and Android pipelines publish independent alpha/preview artifacts.
