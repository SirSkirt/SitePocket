// SitePocket Game Focus Mode. This runs in the website's main frame,
// with NO native IPC or access to local files.
(function () {
  'use strict';
  if (window !== window.top || window.__webcaskGameFocus) return;
  window.__webcaskGameFocus = true;

  var current = null;
  var originalStyle = null;
  var aspect = 1.5;
  var enabled = true;
  var picker = false;
  var highlighted = null;
  var originalOutline = null;
  var badge = null;
  var label = null;
  var attempts = 0;

  function eligible(el) {
    if (!(el instanceof HTMLElement)) return false;
    if (el.closest('[data-webcask-overlay]')) return false;
    var r = el.getBoundingClientRect();
    var style = window.getComputedStyle(el);
    return r.width >= 240 && r.height >= 150 &&
      style.display !== 'none' && style.visibility !== 'hidden';
  }
  function largestGame() {
    var candidates = document.querySelectorAll('canvas, iframe, embed, object');
    var best = null;
    var biggest = 0;
    for (var i = 0; i < candidates.length; i++) {
      var el = candidates[i];
      if (!eligible(el)) continue;
      var r = el.getBoundingClientRect();
      var score = r.width * r.height;
      if (score > biggest) { best = el; biggest = score; }
    }
    return best;
  }
  function styles() {
    if (document.getElementById('webcask-focus-css')) return;
    var sheet = document.createElement('style');
    sheet.id = 'webcask-focus-css';
    sheet.textContent =
      'html[data-webcask-game="on"],html[data-webcask-game="on"] body{' +
      'width:100%!important;height:100%!important;overflow:hidden!important;' +
      'margin:0!important;padding:0!important;background:#000!important}' +
      'html[data-webcask-game="on"] body * {visibility:hidden!important}' +
      'html[data-webcask-game="on"] [data-webcask-target],' +
      'html[data-webcask-game="on"] [data-webcask-target] *,' +
      'html[data-webcask-game="on"] [data-webcask-overlay],' +
      'html[data-webcask-game="on"] [data-webcask-overlay] *{visibility:visible!important}';
    (document.head || document.documentElement).appendChild(sheet);
  }
  function putStyle(el, key, value) {
    el.style.setProperty(key, value, 'important');
  }
  function resize() {
    if (!current || !enabled) return;
    var width = window.innerWidth;
    var height = window.innerHeight;
    if (width < 10 || height < 10) return;
    var displayWidth = Math.min(width, height * aspect);
    var displayHeight = displayWidth / aspect;
    putStyle(current, 'position', 'fixed');
    putStyle(current, 'left', ((width - displayWidth) / 2) + 'px');
    putStyle(current, 'top', ((height - displayHeight) / 2) + 'px');
    putStyle(current, 'width', displayWidth + 'px');
    putStyle(current, 'height', displayHeight + 'px');
    putStyle(current, 'max-width', 'none');
    putStyle(current, 'max-height', 'none');
    putStyle(current, 'margin', '0');
    putStyle(current, 'padding', '0');
    putStyle(current, 'border', '0');
    putStyle(current, 'transform', 'none');
    putStyle(current, 'z-index', '2147483000');
    putStyle(current, 'object-fit', 'contain');
  }
  function restore() {
    document.documentElement.removeAttribute('data-webcask-game');
    if (current) {
      current.removeAttribute('data-webcask-target');
      if (originalStyle === null) current.removeAttribute('style');
      else current.setAttribute('style', originalStyle);
    }
    enabled = false;
    updateBadge('Website view');
  }
  function focus(el) {
    if (!el) { updateBadge('No game canvas or frame found — use Pick'); return; }
    if (current) {
      current.removeAttribute('data-webcask-target');
      if (originalStyle === null) current.removeAttribute('style');
      else current.setAttribute('style', originalStyle);
    }
    current = el;
    originalStyle = el.getAttribute('style');
    var rect = el.getBoundingClientRect();
    aspect = Math.max(0.3, Math.min(6, rect.width / Math.max(rect.height, 1)));
    enabled = true;
    styles();
    el.setAttribute('data-webcask-target', 'true');
    document.documentElement.setAttribute('data-webcask-game', 'on');
    resize();
    updateBadge('Game view');
  }
  function updateBadge(message) {
    if (label) label.textContent = message;
  }
  function overlay() {
    if (!document.body || badge) return;
    badge = document.createElement('div');
    badge.setAttribute('data-webcask-overlay', 'true');
    badge.style.cssText = 'position:fixed!important;top:10px!important;right:10px!important;' +
      'z-index:2147483646!important;padding:7px 9px!important;border-radius:8px!important;' +
      'background:rgba(11,19,26,.86)!important;color:white!important;' +
      'font:12px/1.4 Segoe UI,Arial,sans-serif!important;' +
      'display:flex!important;gap:8px!important;align-items:center!important;' +
      'opacity:.28!important;transition:opacity .2s!important;';
    badge.addEventListener('mouseenter', function () { badge.style.opacity = '1'; });
    badge.addEventListener('mouseleave', function () { badge.style.opacity = '.28'; });
    label = document.createElement('span');
    label.textContent = 'Searching for game…';
    badge.appendChild(label);
    function button(caption, handler) {
      var b = document.createElement('button');
      b.textContent = caption;
      b.style.cssText = 'color:#fff!important;background:#355b69!important;' +
        'border:0!important;border-radius:4px!important;padding:3px 7px!important;' +
        'font:inherit!important;cursor:pointer!important;';
      b.addEventListener('click', function (e) {
        e.stopPropagation();
        handler();
      });
      badge.appendChild(b);
    }
    button('Toggle', function () {
      if (enabled && current) restore();
      else if (current && document.contains(current)) focus(current);
      else focus(largestGame());
    });
    button('Pick', startPicker);
    document.body.appendChild(badge);
  }
  function stopPicker() {
    picker = false;
    if (highlighted) highlighted.style.outline = originalOutline;
    highlighted = null;
    window.removeEventListener('pointermove', movePicker, true);
    window.removeEventListener('click', clickPicker, true);
    updateBadge(enabled && current ? 'Game view' : 'Website view');
  }
  function movePicker(e) {
    var el = document.elementFromPoint(e.clientX, e.clientY);
    if (el && el.closest('[data-webcask-overlay]')) return;
    if (el === highlighted) return;
    if (highlighted) highlighted.style.outline = originalOutline;
    highlighted = el;
    if (el && el instanceof HTMLElement) {
      originalOutline = el.style.outline;
      el.style.outline = '3px solid #65e2bb';
    }
  }
  function clickPicker(e) {
    if (e.target.closest && e.target.closest('[data-webcask-overlay]')) return;
    e.preventDefault(); e.stopImmediatePropagation();
    var choice = highlighted || e.target;
    if (highlighted) highlighted.style.outline = originalOutline;
    highlighted = null;
    stopPicker();
    if (choice instanceof HTMLElement) focus(choice);
  }
  function startPicker() {
    if (picker) { stopPicker(); return; }
    if (enabled) restore();
    picker = true;
    updateBadge('Click the game region to focus');
    window.addEventListener('pointermove', movePicker, true);
    window.addEventListener('click', clickPicker, true);
  }
  window.addEventListener('keydown', function (event) {
    if (!event.ctrlKey || !event.shiftKey || event.altKey) return;
    var key = event.key.toLowerCase();
    if (key === 'g') {
      event.preventDefault();
      event.stopImmediatePropagation();
      if (picker) stopPicker();
      if (enabled && current) restore();
      else if (current && document.contains(current)) focus(current);
      else focus(largestGame());
    } else if (key === 'p') {
      event.preventDefault(); event.stopImmediatePropagation(); startPicker();
    }
  }, true);
  window.addEventListener('resize', resize);
  function check() {
    if (current || picker) return;
    attempts++;
    var game = largestGame();
    if (game) focus(game);
    else if (attempts >= 30) updateBadge('No game detected — use Pick');
  }
  function init() {
    overlay();
    check();
    var timer = window.setInterval(function () {
      if (current || attempts >= 30) { window.clearInterval(timer); return; }
      check();
    }, 1100);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init, { once:true });
  else init();
})();
