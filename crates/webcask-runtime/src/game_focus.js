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
  var lastViewport = '';
  var lastScan = 0;
  var scanQueued = false;
  var scheduled = false;
  function scheduleResize() {
    if (scheduled) return;
    scheduled = true;
    window.requestAnimationFrame(function () {
      scheduled = false;
      resize();
    });
  }
  // Native window resize can precede WebView2's viewport resize by a frame.
  // Poll geometry while focused to also handle fullscreen transitions and
  // game engines that replace their canvas while running.
  window.__webcaskRescale = function () {
    scheduleResize();
    window.setTimeout(scheduleResize, 80);
    window.setTimeout(scheduleResize, 280);
  };

  function eligible(el) {
    if (!(el instanceof HTMLElement)) return false;
    if (el.closest('[data-webcask-overlay]')) return false;
    var r = el.getBoundingClientRect();
    var style = window.getComputedStyle(el);
    // Small Android viewports / split-screen games can be below 240x150.
    return r.width >= 120 && r.height >= 80 &&
      style.display !== 'none' && style.visibility !== 'hidden';
  }
  function gameRoot(el) {
    // Keep HUD/menu elements that share a game container with the canvas.
    // Never select document.body: that would make Game View a no-op.
    var ancestor = el.parentElement;
    for (var depth = 0; ancestor && depth < 5; depth++, ancestor = ancestor.parentElement) {
      if (ancestor === document.body || ancestor === document.documentElement) break;
      if (ancestor.matches('[data-game-root], #game, #game-root, #game-container, #game-wrapper, #game-stage, .game-root, .game-container, .game-wrapper, .game-stage, .game-viewport') &&
          eligible(ancestor)) return ancestor;
    }
    return el;
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
    if (best) return gameRoot(best);
    // DOM-only games may draw inside a named game root rather than a canvas.
    var roots = document.querySelectorAll(
      '[data-game-root], #game-root, #game-container, #game-wrapper, #game-stage, .game-root, .game-container, .game-wrapper, .game-stage, .game-viewport');
    for (var j = 0; j < roots.length; j++) {
      var root = roots[j];
      if (!eligible(root)) continue;
      var size = root.getBoundingClientRect();
      var area = size.width * size.height;
      if (area > biggest) { best = root; biggest = area; }
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
    lastViewport = '';
    window.__webcaskRescale();
    updateBadge('Game view');
  }
  function updateBadge(message) {
    if (label && label.textContent !== message) label.textContent = message;
  }
  function overlay() {
    if (!document.body || badge) return;
    badge = document.createElement('div');
    badge.setAttribute('data-webcask-overlay', 'true');
    badge.style.cssText = 'position:fixed!important;top:10px!important;left:10px!important;' +
      'z-index:2147483646!important;padding:7px 9px!important;border-radius:8px!important;' +
      'background:rgba(11,19,26,.86)!important;color:white!important;' +
      'font:12px/1.4 Segoe UI,Arial,sans-serif!important;' +
      'display:flex!important;gap:8px!important;align-items:center!important;' +
      'opacity:.82!important;transition:opacity .2s!important;';
    badge.addEventListener('mouseenter', function () { badge.style.opacity = '1'; });
    badge.addEventListener('mouseleave', function () { badge.style.opacity = '.82'; });
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
  window.addEventListener('resize', window.__webcaskRescale);
  if (window.visualViewport) window.visualViewport.addEventListener('resize', window.__webcaskRescale);
  function check() {
    // Game engines often mount, replace or resize their canvas AFTER DOM
    // readiness. Continue discovery throughout the session instead of giving
    // up after thirty scans or holding onto a detached rendering surface.
    if (badge && !document.contains(badge)) { badge = null; label = null; }
    overlay();
    if (current && !document.contains(current)) {
      current = null;
      originalStyle = null;
      document.documentElement.removeAttribute('data-webcask-game');
      updateBadge('Game view: rendering surface changed; searching…');
    }
    if (current) {
      if (enabled) {
        styles();
        var viewport = window.innerWidth + 'x' + window.innerHeight;
        if (viewport !== lastViewport) {
          lastViewport = viewport;
          window.__webcaskRescale();
        }
      }
      return;
    }
    if (picker || !enabled) return;
    var game = largestGame();
    if (game) focus(game);
    else updateBadge('Waiting for game canvas · Pick');
  }
  function scheduleCheck() {
    if (scanQueued || current || !enabled) return;
    scanQueued = true;
    window.setTimeout(function () {
      scanQueued = false;
      check();
    }, 150);
  }
  function init() {
    // Wry injects at document-start. document.documentElement can still be
    // null then: calling ResizeObserver.observe(null) aborted Game View on
    // BOTH WebView2 and Android before DOMContentLoaded was registered.
    if (!document.documentElement || !document.body) {
      document.addEventListener('DOMContentLoaded', init, { once: true });
      return;
    }
    overlay();
    check();
    // Register observers ONLY after the HTML root actually exists. They are
    // optional; if blocked, low-cost periodic detection still works.
    if (window.ResizeObserver) {
      try {
        var resizeObserver = new ResizeObserver(window.__webcaskRescale);
        resizeObserver.observe(document.documentElement);
        window.__webcaskFocusResizeObserver = resizeObserver;
      } catch (_) { /* no impact on Game View startup */ }
    }
    // Keep a low-cost safety poll: games may resize without DOM mutations.
    window.setInterval(check, 900);
    if (window.MutationObserver) {
      try {
        var observer = new MutationObserver(scheduleCheck);
        observer.observe(document.documentElement, { childList: true, subtree: true });
        window.__webcaskFocusMutationObserver = observer;
      } catch (_) { /* fall back to the interval */ }
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init, { once:true });
  else init();
})();
