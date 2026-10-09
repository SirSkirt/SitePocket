// Resource-discovery only. No native filesystem or arbitrary command access.
// Start the app, explore levels so lazy assets are requested, then Save Offline.
(function () {
  'use strict';
  if (window.__webcaskCapturing) return;
  window.__webcaskCapturing = true;
  var seen = new Set();
  var saved = false;
  var panel = null;
  var info = null;
  var timer = 0;
  function send(payload) {
    if (window.ipc && typeof window.ipc.postMessage === 'function')
      window.ipc.postMessage(JSON.stringify(payload));
  }
  function collect() {
    try {
      performance.getEntriesByType('resource').forEach(function (item) { add(item.name); });
      document.querySelectorAll('script[src],link[href],img[src],video[src],audio[src],source[src],iframe[src],embed[src]').forEach(function (el) {
        add(el.src || el.href);
      });
    } catch (e) {}
    var batch = Array.from(seen);
    if (batch.length) {
      batch.slice(0, 3000).forEach(function (url) { seen.delete(url); });
      send({ kind: 'resources', urls: batch.slice(0, 3000) });
    }
  }
  function add(url) {
    if (!url || seen.size > 3000) return;
    try { seen.add(new URL(url, location.href).href); } catch (e) {}
  }
  function finish() {
    if (saved) return;
    collect();
    saved = true;
    send({ kind: 'finish' });
    if (info) info.textContent = 'Downloading files… please wait.';
  }
  window.__webcaskCaptureStatus = function (message) {
    if (info) info.textContent = message;
    saved = false;
  };
  function start() {
    if (window !== window.top || panel) return;
    panel = document.createElement('aside');
    panel.setAttribute('data-webcask-overlay', 'true');
    panel.style.cssText = 'position:fixed!important;bottom:12px!important;left:12px!important;' +
      'z-index:2147483647!important;background:#10202fea!important;border:1px solid #5abca0!important;' +
      'color:#e8fff5!important;padding:12px!important;border-radius:10px!important;' +
      'font:12px Segoe UI,Arial,sans-serif!important;width:280px!important;display:block!important;' +
      'visibility:visible!important;';
    var title = document.createElement('b');
    title.textContent = 'WebCask · Offline Capture';
    title.style.cssText = 'display:block!important;margin-bottom:6px!important;';
    info = document.createElement('div');
    info.textContent = 'Play through the game to load its assets. Then save.';
    info.style.cssText = 'margin-bottom:9px!important;line-height:1.4!important;';
    var button = document.createElement('button');
    button.textContent = 'Save Offline Candidate';
    button.style.cssText = 'background:#6ed8b2!important;color:#10252d!important;' +
      'border:0!important;padding:8px 12px!important;border-radius:6px!important;' +
      'cursor:pointer!important;font:600 12px Segoe UI,Arial,sans-serif!important;';
    button.addEventListener('click', finish);
    panel.append(title, info, button);
    document.body.appendChild(panel);
    collect();
    timer = setInterval(collect, 1800);
    try {
      new PerformanceObserver(function (entries) {
        entries.getEntries().forEach(function (entry) { add(entry.name); });
      }).observe({ type: 'resource', buffered: true });
    } catch (e) {}
  }
  window.addEventListener('keydown', function (e) {
    if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 's') {
      e.preventDefault(); e.stopImmediatePropagation(); finish();
    }
  }, true);
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, {once:true});
  else start();
})();
