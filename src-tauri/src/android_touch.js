// WebCask Android Touch Layer — injected into the game WebView itself.
// No Tauri commands, filesystem bridge, eval of user-supplied values, or third-party dependencies.
// Events are DOM KeyboardEvents. Games requiring trusted/native KeyEvents may not respond.
(function () {
  'use strict';
  const allowedOrigin = __ALLOWED_ORIGIN__;
  const buttons = __BUTTONS_JSON__;
  const enabled = __ENABLED__;
  if (!enabled || window !== window.top || location.origin !== allowedOrigin
      || window.__webcaskTouchLoaded) return;
  window.__webcaskTouchLoaded = true;

  const codeToKey = (code) => {
    if (/^Key[A-Z]$/.test(code)) return code[3].toLowerCase();
    if (/^Digit[0-9]$/.test(code)) return code[5];
    return ({
      Space: ' ', Enter: 'Enter', Escape: 'Escape', Tab: 'Tab',
      ArrowLeft: 'ArrowLeft', ArrowRight: 'ArrowRight',
      ArrowUp: 'ArrowUp', ArrowDown: 'ArrowDown',
      ShiftLeft: 'Shift', ControlLeft: 'Control',
      AltLeft: 'Alt', Backspace: 'Backspace',
    })[code] || code;
  };
  const legacyCode = (code) => {
    if (/^Key[A-Z]$/.test(code)) return code.charCodeAt(3);
    if (/^Digit[0-9]$/.test(code)) return code.charCodeAt(5);
    return ({
      ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39,
      Space: 32, Enter: 13, Escape: 27, Tab: 9,
      ShiftLeft: 16, ControlLeft: 17, AltLeft: 18, Backspace: 8,
    })[code] || 0;
  };
  const downCounts = new Map();
  const heldPointers = new Map();
  function keyEvent(code, down) {
    const key = codeToKey(code);
    const event = new KeyboardEvent(down ? 'keydown' : 'keyup', {
      key, code, bubbles: true, cancelable: true, composed: true,
      shiftKey: code === 'ShiftLeft', ctrlKey: code === 'ControlLeft',
      altKey: code === 'AltLeft'
    });
    const num = legacyCode(code);
    for (const property of ['keyCode', 'which']) {
      try { Object.defineProperty(event, property, { get: () => num }); } catch (_) {}
    }
    const target = document.activeElement instanceof HTMLElement &&
      document.activeElement !== document.body
        ? document.activeElement : document.querySelector('canvas') || document.body;
    (target || document).dispatchEvent(event);
  }
  function press(code) {
    const count = downCounts.get(code) || 0;
    downCounts.set(code, count + 1);
    if (!count) keyEvent(code, true);
  }
  function release(code) {
    const count = downCounts.get(code) || 0;
    if (count <= 1) {
      downCounts.delete(code);
      if (count) keyEvent(code, false);
    } else downCounts.set(code, count - 1);
  }
  function releaseAll() {
    for (const code of downCounts.keys()) keyEvent(code, false);
    downCounts.clear(); heldPointers.clear();
  }
  window.addEventListener('blur', releaseAll);
  document.addEventListener('visibilitychange', () => { if (document.hidden) releaseAll(); });

  function attach() {
    if (!document.body || document.getElementById('__webcask_touch_root')) return;
    const root = document.createElement('div');
    root.id = '__webcask_touch_root';
    root.setAttribute('data-webcask-overlay', 'true');
    root.style.cssText =
      'position:fixed!important;inset:0!important;z-index:2147483600!important;' +
      'pointer-events:none!important;visibility:visible!important;' +
      'font-family:system-ui,sans-serif!important;';
    const controls = document.createElement('div');
    controls.style.cssText = 'position:absolute!important;inset:0!important;pointer-events:none!important;';
    const gameButton = (spec) => {
      const el = document.createElement('button');
      el.type = 'button';
      el.textContent = spec.label;
      el.setAttribute('aria-label', spec.label + ' — ' + spec.key);
      const size = Number(spec.size) || 56;
      el.style.cssText =
        'position:absolute!important;left:' + Number(spec.x) + '%!important;' +
        'top:' + Number(spec.y) + '%!important;transform:translate(-50%,-50%)!important;' +
        'width:' + size + 'px!important;height:' + size + 'px!important;' +
        'border:2px solid rgba(255,255,255,.5)!important;border-radius:50%!important;' +
        'background:rgba(18,39,54,.43)!important;color:#fff!important;' +
        'font:bold 13px system-ui,sans-serif!important;pointer-events:auto!important;' +
        'touch-action:none!important;user-select:none!important;' +
        '-webkit-user-select:none!important;visibility:visible!important;' +
        'z-index:2147483601!important;padding:0!important;outline:0!important;';
      const finish = e => {
        const lastCode = heldPointers.get(e.pointerId);
        if (!lastCode) return;
        heldPointers.delete(e.pointerId);
        release(lastCode);
        el.style.background = 'rgba(18,39,54,.43)';
        e.preventDefault(); e.stopPropagation();
      };
      el.addEventListener('pointerdown', e => {
        if (heldPointers.has(e.pointerId)) return;
        heldPointers.set(e.pointerId, spec.key);
        try { el.setPointerCapture(e.pointerId); } catch (_) {}
        press(spec.key);
        el.style.background = 'rgba(85,200,170,.76)';
        e.preventDefault(); e.stopPropagation();
      });
      ['pointerup', 'pointercancel', 'lostpointercapture'].forEach(type => {
        el.addEventListener(type, finish);
      });
      el.addEventListener('contextmenu', e => e.preventDefault());
      controls.appendChild(el);
    };
    buttons.forEach(gameButton);
    const toggle = document.createElement('button');
    toggle.type = 'button';
    toggle.textContent = '🎮';
    toggle.setAttribute('aria-label', 'Toggle WebCask touch controls');
    toggle.style.cssText =
      'position:absolute!important;top:12px!important;right:12px!important;' +
      'width:36px!important;height:36px!important;border-radius:10px!important;' +
      'background:rgba(16,35,48,.62)!important;color:#fff!important;' +
      'border:1px solid #9bc!important;visibility:visible!important;' +
      'pointer-events:auto!important;touch-action:manipulation!important;' +
      'z-index:2147483602!important;';
    let visible = true;
    toggle.addEventListener('click', () => {
      visible = !visible;
      controls.style.display = visible ? 'block' : 'none';
      if (!visible) releaseAll();
      toggle.style.opacity = visible ? '1' : '.45';
    });
    root.append(controls, toggle);
    document.body.appendChild(root);
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', attach, {once:true});
  else attach();
  // Browser games may replace their body element after loading.
  const observer = new MutationObserver(() => {
    if (!document.getElementById('__webcask_touch_root')) attach();
  });
  observer.observe(document.documentElement, {childList:true, subtree:false});
})();