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
  let gameDialogOpen = false;
  function showGameError(kind, message, detail) {
    if (gameDialogOpen) return;
    const install = () => {
      if (!document.body || gameDialogOpen) return;
      gameDialogOpen = true;
      const backdrop = document.createElement('div');
      backdrop.setAttribute('data-webcask-error', 'true');
      backdrop.style.cssText =
        'position:fixed!important;inset:0!important;z-index:2147483646!important;' +
        'background:rgba(3,12,22,.90)!important;display:flex!important;' +
        'align-items:center!important;justify-content:center!important;padding:18px!important;';
      const dialog = document.createElement('section');
      dialog.style.cssText =
        'box-sizing:border-box!important;width:min(540px,100%)!important;' +
        'max-height:85vh!important;overflow:auto!important;border-radius:15px!important;' +
        'border:1px solid #416277!important;background:#1b3040!important;' +
        'box-shadow:0 14px 65px #000b!important;padding:22px!important;' +
        'color:#e1f3f3!important;font:13px system-ui,sans-serif!important;' +
        'text-align:left!important;';
      const tag = document.createElement('div');
      tag.textContent = 'WEBCASK DEBUGGER';
      tag.style.cssText = 'font:bold 10px system-ui!important;color:#81d9c0!important;letter-spacing:2px!important;';
      const heading = document.createElement('h2');
      heading.textContent = 'Game error detected';
      heading.style.cssText = 'color:#fff!important;font:bold 20px system-ui!important;margin:10px 0!important;';
      const messageEl = document.createElement('p');
      messageEl.textContent = String(message || 'Unknown game error').slice(0, 500);
      messageEl.style.cssText = 'color:#ffb0a9!important;line-height:1.5!important;overflow-wrap:anywhere!important;';
      const details = document.createElement('pre');
      details.textContent = String(detail || '').slice(0, 1200);
      details.style.cssText = 'white-space:pre-wrap!important;overflow-wrap:anywhere!important;' +
        'background:#0e1f2f!important;padding:12px!important;border-radius:8px!important;' +
        'color:#bcd0dc!important;font:11px monospace!important;max-height:35vh!important;overflow:auto!important;';
      const note = document.createElement('p');
      note.textContent = 'This is a game-page error. The full timeline is saved in SitePocket and can be copied after you return to the manager.';
      note.style.cssText = 'color:#9dbacb!important;font:12px system-ui!important;line-height:1.5!important;';
      const close = document.createElement('button');
      close.textContent = 'Close debugger';
      close.style.cssText = 'padding:12px 18px!important;background:#6fdbb8!important;color:#09282e!important;' +
        'border:none!important;border-radius:8px!important;font:bold 12px system-ui!important;';
      close.addEventListener('click', () => { backdrop.remove(); gameDialogOpen = false; });
      dialog.append(tag, heading, messageEl, details, note, close);
      backdrop.appendChild(dialog);
      document.body.appendChild(backdrop);
    };
    if (document.body) install();
    else document.addEventListener('DOMContentLoaded', install, {once:true});
  }

  function send(kind, message, detail = '') {
    if (remaining-- <= 0) return;
    // Never capture full page contents, cookies, form values, or game storage.
    const payload = JSON.stringify({
      kind, message: String(message || '').slice(0, 500),
      detail: String(detail || '').slice(0, 1800)
    });
    if (kind === 'script-error' || kind === 'promise-error') {
      showGameError(kind, message, detail);
    }
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