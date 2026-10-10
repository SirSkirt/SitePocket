// WebCask local-game debugger. Runs inside the game WebView with NO
// SitePocket native IPC permissions. Reports go only to its localhost server.
// Errors and diagnostics are bounded and persist in Rust across app restarts.
(() => {
  'use strict';
  const allowedOrigin = __WEBCASK_ALLOWED_ORIGIN__;
  if (location.origin !== allowedOrigin || window !== window.top) return;
  if (window.__webcaskDiagnosticInstalled) return;
  window.__webcaskDiagnosticInstalled = true;

  let remaining = 32;
  const endpoint = allowedOrigin + '/__webcask_debug';
  function send(kind, message, detail = '') {
    if (remaining-- <= 0) return;
    // Never capture full page contents, cookies, form values, or game storage.
    const payload = JSON.stringify({
      kind, message: String(message || '').slice(0, 500),
      detail: String(detail || '').slice(0, 1800)
    });
    try {
      fetch(endpoint, {
        method: 'POST', mode: 'same-origin', credentials: 'omit',
        headers: { 'Content-Type': 'text/plain' },
        body: payload, keepalive: true
      }).catch(() => {});
    } catch (_) {}
  }
  window.addEventListener('error', event => {
    if (event.target && event.target !== window) {
      const target = event.target;
      if (target instanceof HTMLElement) {
        send('resource-error', 'A web resource failed to load',
          (target.getAttribute('src') || target.getAttribute('href') || target.tagName).slice(0, 500));
      }
      return;
    }
    send('script-error', event.message || 'Uncaught JavaScript exception',
      [event.filename || '', event.lineno || '', event.colno || '',
       event.error && event.error.stack || ''].join(' | '));
  }, true);
  window.addEventListener('unhandledrejection', event => {
    const reason = event.reason;
    send('promise-error', 'Unhandled game promise rejection',
      reason && (reason.stack || reason.message) || String(reason));
  });
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => send('page-ready', 'DOM content loaded'), {once:true});
  } else {
    send('page-ready', 'DOM already ready');
  }
  try {
    const watcher = new PerformanceObserver(list => {
      for (const entry of list.getEntries()) {
        if (entry.duration > 2000) {
          send('long-task', 'Game main thread blocked for over 2 seconds',
            Math.round(entry.duration) + ' ms');
          break;
        }
      }
    });
    watcher.observe({type:'longtask', buffered:false});
  } catch (_) { /* Long-task API is optional on Android WebView. */ }
})();