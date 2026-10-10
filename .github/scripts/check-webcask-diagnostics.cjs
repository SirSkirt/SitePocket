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
console.log('WebCask diagnostics isolation tests passed.');
