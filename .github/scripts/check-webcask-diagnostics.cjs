// Run without Android hardware to check that our page diagnostics do not
// spontaneously report GAME errors and that opaque exceptions stay unattributed.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const script = fs.readFileSync('src-tauri/src/android_debug.js', 'utf8')
  .replace('__WEBCASK_ALLOWED_ORIGIN__', JSON.stringify('http://127.0.0.1:41000'));

function boot(brokenFetch = false) {
  const handlers = {};
  const requests = [];
  const warnings = [];
  const window = {
    addEventListener(name, handler) {
      (handlers[name] ||= []).push(handler);
    }
  };
  window.top = window;
  const document = {
    body: null,
    readyState: 'loading',
    addEventListener() {}
  };
  const fetch = (_url, init) => {
    if (brokenFetch) throw Error('synthetic failure from the WebCask test harness');
    requests.push(JSON.parse(init.body));
    return Promise.resolve({});
  };
  vm.runInNewContext(script, {
    window, document, fetch,
    location: { origin: 'http://127.0.0.1:41000' },
    console: { warn: message => warnings.push(message) },
    HTMLElement: class {}, PerformanceObserver: undefined,
  }, { timeout: 1000, filename: 'android_debug.js' });
  return {
    requests, warnings,
    emit(name, event) { for (const handler of handlers[name] || []) handler(event); }
  };
}

const normal = boot();
assert.equal(normal.requests.length, 0, 'diagnostics itself must not generate a script error');
normal.emit('error', {
  target: {}, message: 'Script error.', filename: '',
  lineno: 0, colno: 0, error: null
});
assert.equal(normal.requests.length, 0, 'non-window errors are resources, not game exceptions');
normal.emit('error', {
  target: normal === undefined ? undefined : null,
  message: 'Script error.', filename: '', lineno: 0, colno: 0, error: null
});
assert.equal(normal.requests[0].kind, 'opaque-script-error');
assert.match(normal.requests[0].detail, /Native console/);

normal.emit('error', {
  target: null, message: 'ReferenceError: gameFault is not defined',
  filename: 'http://127.0.0.1:41000/game.js', lineno: 17, colno: 3,
  error: { stack: 'at game.js:17:3' }
});
assert.equal(normal.requests[1].kind, 'script-error');
assert.match(normal.requests[1].detail, /game.js/);

const broken = boot(true);
broken.emit('error', {
  target: null, message: 'Script error.', filename: '',
  lineno: 0, colno: 0, error: null
});
assert.equal(broken.warnings.length, 1);
assert.match(broken.warnings[0], /WebCask diagnostics internal failure/);

const nativeRustBridge = fs.readFileSync('crates/tauri-plugin-android-zip-import/src/lib.rs', 'utf8');
const nativeKotlinBridge = fs.readFileSync(
  'crates/tauri-plugin-android-zip-import/android/src/main/java/dev/sitepocket/zipimport/ZipImportPlugin.kt',
  'utf8'
);
assert.match(nativeRustBridge,
  /#\[serde\(rename_all = "camelCase"\)\]\s*struct ActiveGameRequest\s*\{\s*app_id: String\s*\}/,
  'Rust ActiveGameRequest must serialize the appId field expected by Kotlin');
assert.match(nativeKotlinBridge, /var appId: String/,
  'Kotlin ActiveGameArgs must accept the camelCase appId field');

// Rotation must NOT recreate a live game WebView. The Android build script
// declares handling for orientation and size changes on the secondary
// WebCaskActivity (MainActivity settings do not carry over).
const androidSetup = fs.readFileSync('.github/scripts/setup-webcask-android.py', 'utf8');
const nativeActivity = fs.readFileSync('src-tauri/android/WebCaskActivity.kt', 'utf8');
const managerUI = fs.readFileSync('src/main.ts', 'utf8');
assert.match(androidSetup, /player_configs\s*=\s*\(/,
  'Android generated WebCaskActivity needs explicit configChanges');
for (const flag of ['orientation', 'screenSize', 'smallestScreenSize', 'screenLayout']) {
  assert.ok(androidSetup.includes(flag), 'Activity must handle configuration: ' + flag);
}
assert.match(nativeActivity, /override fun onConfigurationChanged\(newConfig: Configuration\)/,
  'Activity must respond to configuration changes without restarting the WebView');
assert.match(nativeActivity, /playerWebView\?\.invalidate\(\)/,
  'Activity must redraw its WebView after rotation');
assert.match(nativeActivity, /"native-tauri-injection-conflict"/,
  'Tauri bootstrap errors must not be labeled as game failures');
assert.match(managerUI, /event\.code === 'native-tauri-injection-conflict'/,
  'SitePocket must explain known Tauri-only bootstrap errors');

console.log('WebCask diagnostics isolation tests passed.');
