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
**Import game**. The selected `content://` document is copied using the native Android
ContentResolver into app-private cache (the original ZIP is untouched), extracted with the same bounded
Rust importer as Windows, and appears in the library. Launch it through a
read-only `127.0.0.1` local asset server. It supports game-relative and
absolute asset paths, correct WASM content types, and runs without internet
provided every game resource is inside the ZIP. Imported games get the same
per-game editable touchscreen keyboard mappings.

**Updating Android previews:** Starting with the new Android preview signer,
GitHub Actions will use a single development-only signing certificate and
an increasing Android `versionCode` for each workflow run. APKs signed with
that same certificate can install directly over older *fixed-signer* builds,
without removing imported games or settings. **One uninstall is required to
switch from previews 1–6**, because those used different ephemeral GitHub
runner signing certificates. Export or preserve any local game ZIPs before
that initial uninstall; Android app data will be erased. The preview signing
key is included solely for reproducible **non-production testing**. Because
this key is stored in source control, it is **not secure for production
distribution**; replace it with a privately held release key using GitHub
encrypted secrets before any public/Play Store release.

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

### Mobile ZIP troubleshooting

If Android Files supplies a `content://` URI (e.g. Downloads), SitePocket
uses an Android-only Kotlin plugin to call `ContentResolver.openInputStream`.
This is required because Tauri filesystem calls do not accept those document
URIs as real paths. The import is limited to 1 GiB compressed and reuses the
Windows archive validation (ZIP slip/bomb guards and `index.html` detection).
If an Android provider revokes access, reselect the ZIP from Android Files.
Android downloads and cloud providers may require the document to be locally
available before importing. Offline playback works for self-contained games;
remote assets and authentication cannot be magically included in a ZIP.

SitePocket Android's launcher icon is generated from the **same original
Windows icon** instead of the temporary Android placeholder. This ensures the
app launcher uses consistent branding on both platforms.


## Error-only WebCask debugger (Android alpha)

The SitePocket debugger **stays hidden until an error is detected** in an
Android game or its WebCask launch path. When this happens, a matching
SitePocket-styled dialog appears, explaining the likely failing layer and
showing recent diagnostic events. It includes **Copy Report**, **Clear Report**
and **Close** controls. Dismissing the dialog suppresses previously acknowledged
errors but future errors open it again.

What it can observe:
- Native WebCask launcher and Android WebView creation errors, navigation
  start/finish events, local HTTP game-server startup.
- JavaScript exceptions and unhandled promise rejections in the game's
  **top-level page**; errors loading scripts, media, or other assets; missing
  local JS/CSS/WASM/data files.
- Optional browser Long Tasks (2-second threshold; warning only).
- Startup watchdog: if **no DOM-content-loaded readiness signal arrives
  within 22 seconds**, the debugger reports a **possible startup stall**.
  This alone does **not** establish which component is at fault.

A bounded history of the last 100 events is persisted in the app's private
data directory so it is available after restarting SitePocket following a
freeze. The manager polls the backend for new errors approximately every
2.5 seconds. A completely frozen Android Activity cannot show a popup
until the manager becomes responsive again or is restarted; Android process
crashes can also prevent the final event from being recorded.

The injected script uses a same-origin localhost reporting endpoint, NOT
Tauri native commands. Requests are limited to 4 KiB and checked against
the WebCask localhost origin. No personal files, cookies, or browser storage
are collected by the debugger. Report text may nevertheless contain file names,
error URLs or game stack traces, so review before sharing.

Important limitations: page-origin JavaScript cannot observe errors from
cross-origin frames, blocked scripts, hard Android WebView process crashes,
or native Kotlin stack traces. The exact root cause still needs testing on
a device; the error classification is guidance, not a verdict.

## Android player launch fix (after debugger report)

A real Android BlockBend report showed:
- Local game server listening on 127.0.0.1:41000.
- Android game launch began.
- No `webview-created` or navigation events arrived before the 22-second timeout.

This points to Android window/Activity creation, rather than a confirmed
game JavaScript failure. WebCask now registers a dedicated
`WebCaskActivity` for its second Android WebView window, configures the
AndroidX Window/Startup Activity Embedding support, and calls
`WebviewWindowBuilder::activity_name("WebCaskActivity")`. It also launches
the player from an async Tauri command on a blocking-worker thread so the
manager's IPC/GUI can remain responsive if Android waits for its Activity
creation. A new `android-activity-build-requested` diagnostic marks the
exact boundary before `builder.build()`.

This is a targeted fix based on the diagnostic evidence, not proof the freeze
is solved. On-device validation still needs to confirm that an Android
Activity and WebView are created and that BlockBend loads its first page.
The imported ZIP, saved games, controls, and desktop WebCask code are not
changed by this fix.

For Tauri Android multi-window requirements see
https://v2.tauri.app/learn/mobile-multiwindow/.

## Native Android WebView debugger — independent of game scripts

WebCask also records Android-side diagnostics directly in Kotlin:
- Activity creation requested, `onCreate` completed, resumed/paused/destroyed.
- WebView object creation and loading-progress milestones.
- JavaScript console **errors and warnings** through Android WebChromeClient
  (even if the injected WebCask page script never starts).
- Android WebView renderer unresponsive/recovered callbacks.
- A separate native Activity UI-thread heartbeat; a gap longer than about
  12 seconds generates an error, even when the JS runtime is blocked.

The native journal is bounded (under approximately 180 KiB), stored inside
SitePocket's private Android files directory, and printed under the
`WebCaskNative` Android Logcat tag. A restricted native plugin bridge reads
these entries from Kotlin into the **same SitePocket error-only debugger**
with the current game's ID. The manager can show them after a freeze and
restart; the game page is not given any extra native API permissions.

The native logger wraps Tauri's existing WebChromeClient instead of
replacing its file-picker, permissions, popup, fullscreen-media, and
JavaScript-dialog behavior; if the original handler is not yet installed,
WebCask waits to attach its wrapper. It does not log entire pages, request
bodies, authentication headers, or cookies. Native console error text and
script file names **can** contain details of a game; review copied reports
before sharing.

The most useful diagnosis boundary for BlockBend is now:
`android-activity-build-requested` -> `native-activity-create-requested`
-> `native-activity-created` -> `native-webview-created` ->
`native-webview-progress` -> page-ready. If the report stops before
the native Activity begins, creation/registration is suspect. If Android
says its renderer is unresponsive, resource-intensive game code or the
WebView renderer is suspect instead.

## Android diagnostic isolation and pre-navigation native capture

The Android player now opens an empty `about:blank` bootstrap document
first. The native Activity waits for Wry's actual `RustWebChromeClient`,
wraps it with the forwarding WebCask console logger, writes an app-private
`native-console-hook-installed` event, and only then allows Rust to navigate
to the game. A matching **current-launch** journal event is required (game
UUID and timestamp); if it is not observed within seven seconds, the game
navigation is canceled with `native-console-hook-timeout` rather than
silently missing the earliest console errors. This order still requires an
on-device test; a passing compile is not proof of Android activity timing.

To isolate the WebCask page scripts from a game's own code: in Android
**Game Options**, tap **Launch native-only**. This temporary test disables
WebCask's injected touch overlay, game-focus script and page debugger, but
keeps the **native Kotlin console logger** and Rust navigation events. The
normal Launch button restores all usual behaviors; no stored game settings
or imported assets are changed. Compare reports after equivalent navigation
in each mode and note the `injection-mode` stage.

Browser-reported `Script error.` entries with no filename, line or stack
are now labeled `opaque-script-error` with source `unattributed` — neither
the game nor WebCask is proven responsible. If the WebCask diagnostic script
catches one of its own failures, its prefixed native-console message becomes
`native-instrumentation-error`. Report entries are sorted by event time
rather than ingestion order, because native journal events reach Rust later.

The release CI runs `node .github/scripts/check-webcask-diagnostics.cjs` to
regression-test source-less error classification and guard behavior.
Physical-device A/B results are still needed to prove what caused the
original five anonymous errors.
