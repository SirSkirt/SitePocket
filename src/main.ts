import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { BaseDirectory, copyFile, mkdir, remove } from '@tauri-apps/plugin-fs';
import './style.css';

type WindowMode = 'windowed' | 'maximized' | 'fullscreen';
type InstalledApp = { id: string; name: string; kind: 'online' | 'local'; url: string | null; port: number | null; gameFocus?: boolean; entryPath?: string | null; windowMode?: WindowMode; windowWidth?: number; windowHeight?: number; iconData?: string | null; touchEnabled?: boolean; touchButtons?: TouchButton[] };
type TouchButton = { id: string; label: string; key: string; x: number; y: number; size: number };
type Mode = 'website' | 'game';
const onAndroid = /Android/i.test(navigator.userAgent);
const touchKeys = [
  'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space', 'Enter', 'Escape',
  'Tab', 'ShiftLeft', 'ControlLeft', 'AltLeft', 'Backspace',
  ...'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('').map(letter => 'Key' + letter),
  ...'0123456789'.split('').map(num => 'Digit' + num),
];
let apps: InstalledApp[] = [];
let mode: Mode = 'website';
let selectedPath = '';
let busy = false;
let draftName = '';
let draftUrl = '';
let editingId: string | null = null;
let optionsSaving = false;
let optionsError = '';
let iconBusy = false;
let touchEditingId: string | null = null;
let touchDraft: TouchButton[] = [];
let touchDraftEnabled = true;
let touchSaving = false;
let touchError = '';
let touchNextId = 0;
let feedback: { kind: 'info' | 'error' | 'success'; message: string } | null = null;

function formMessage(kind: 'info' | 'error' | 'success', message: string) {
  feedback = { kind, message };
  const target = document.querySelector<HTMLDivElement>('#form-feedback');
  if (target) {
    target.className = 'form-feedback ' + kind;
    target.textContent = message;
  }
}
function clearFormMessage() {
  feedback = null;
  const target = document.querySelector<HTMLDivElement>('#form-feedback');
  if (target) {
    target.textContent = mode === 'game'
      ? (onAndroid ? 'Select an HTML5 game ZIP from your phone.' : 'Import a folder or ZIP with an index.html file.')
      : 'No PWA manifest required.';
    target.className = 'form-feedback';
  }
}

const mount = document.querySelector<HTMLDivElement>('#app')!;
const escapeHtml = (s: string) => s.replace(/[&<>"']/g, c => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' })[c] || c);
const errorText = (e: unknown) => e instanceof Error ? e.message : String(e);
function notice(message: string, bad = false) {
  const toast = document.getElementById('toast')!;
  toast.textContent = message;
  toast.className = bad ? 'toast bad show' : 'toast show';
  window.setTimeout(() => toast.classList.remove('show'), 4800);
}
async function refresh() {
  try { apps = await invoke<InstalledApp[]>('list_apps'); render(); }
  catch (e) { notice(errorText(e), true); }
}
function render() {
  const itemCount = apps.length;
  mount.innerHTML = '<div class="shell">' +
    '<aside class="side"><div class="brand"><div class="logo">S</div><div><strong>SitePocket</strong><small>Powered by WebCask</small></div></div>' +
    '<nav><div class="nav selected">▦ &nbsp; My applications <span>' + itemCount + '</span></div>' +
    '<div class="section-title">INSTALLATION</div><button class="nav ' + (mode === 'website' ? 'selected' : '') + '" data-mode="website">◎ &nbsp; Website</button>' +
    '<button class="nav ' + (mode === 'game' ? 'selected' : '') + '" data-mode="game">▣ &nbsp; Offline game</button></nav>' +
    '<div class="side-foot"><span class="dot"></span> WebCask Runtime <small>v0.1.0 Alpha</small></div></aside>' +
    '<main><header><div><div class="eyebrow">APPLICATION LIBRARY</div><h1>Your web apps. <span>Your rules.</span></h1>' +
    '<p>Run websites and HTML5 games with WebCask on your device.</p></div><div class="version">ALPHA 0.1.0</div></header>' +
    '<section class="install-card"><div class="install-heading"><div class="glyph">' + (mode === 'website' ? '◎' : '▣') + '</div><div>' +
    '<h2>' + (mode === 'website' ? 'Install a website' : 'Import an offline game') + '</h2><p>' +
    (mode === 'website' ? 'Give any website its own dedicated WebCask window.' : 'Bring your HTML, JavaScript or WebAssembly game as a folder or ZIP.') +
    '</p></div></div><form id="install-form"><div class="form-grid"><label>APP NAME<input id="name" placeholder="' +
    (mode === 'website' ? 'e.g. YouTube Music' : 'e.g. My Game') + '" value="' + escapeHtml(draftName) + '" maxlength="80" required /></label>' +
    (mode === 'website' ?
    '<label>WEBSITE ADDRESS<input id="address" type="url" placeholder="https://example.com" value="' + escapeHtml(draftUrl) + '" required /></label>' :
    '<label>GAME SOURCE<div class="picker"><span id="path-label">' +
    (selectedPath ? (onAndroid ? 'Android ZIP selected' : escapeHtml(selectedPath)) : 'No game selected') +
    '</span>' + (onAndroid ? '' : '<button type="button" class="secondary" id="pick-folder">Folder</button>') +
    '<button type="button" class="secondary" id="pick-zip">' + (onAndroid ? 'Choose ZIP' : 'ZIP') +
    '</button></div></label>') +
    '</div><div class="form-foot"><div id="form-feedback" role="status" aria-live="polite" class="form-feedback ' +
    (feedback ? feedback.kind : '') + '">' + escapeHtml(feedback?.message ||
    (mode === 'game' ? (onAndroid ? 'Select an HTML5 game ZIP from your phone.' :
    'Import a folder or ZIP with an index.html file.') : 'No PWA manifest required.')) +
    '</div><button id="install-button" class="primary" type="submit" ' + (busy ? 'disabled' : '') + '>' +
    (busy ? 'Installing…' : mode === 'website' ? '＋ Install website' : '＋ Import game') + '</button></div></form></section>' +
    '<section class="library-head"><div><div class="eyebrow">READY TO LAUNCH</div><h2>Installed applications <span>' + itemCount + '</span></h2></div><button class="secondary" id="refresh-list">↻ Refresh library</button></section>' +
    '<div class="library">' + (itemCount ? apps.map(renderApp).join('') :
    '<div class="empty"><div class="empty-icon">▦</div><h3>Nothing in your pocket yet</h3><p>Install your first site or import a game to get started.</p></div>') +
    '</div><footer>SitePocket manages your apps · WebCask runs them independently · ' +
    (onAndroid ? 'Android Preview' : 'Windows Alpha') + '</footer></main></div>' +
    renderOptions() + renderTouchEditor() + '<div id="toast" class="toast"></div>';
  document.querySelectorAll<HTMLButtonElement>('[data-mode]').forEach(button => button.onclick = () => {
    mode = button.dataset.mode as Mode;
    selectedPath = ''; draftName = ''; draftUrl = ''; feedback = null;
    render();
  });
  document.getElementById('install-form')!.addEventListener('submit', submitInstall);
  document.querySelector<HTMLInputElement>('#name')?.addEventListener('input', e => {
    draftName = (e.target as HTMLInputElement).value;
    if (feedback?.kind === 'error') clearFormMessage();
  });
  document.querySelector<HTMLInputElement>('#address')?.addEventListener('input', e => {
    draftUrl = (e.target as HTMLInputElement).value;
    if (feedback?.kind === 'error') clearFormMessage();
  });
  document.getElementById('refresh-list')?.addEventListener('click', refresh);
  document.getElementById('pick-folder')?.addEventListener('click', () => pick(true));
  document.getElementById('pick-zip')?.addEventListener('click', () => pick(false));
  document.querySelectorAll<HTMLButtonElement>('[data-action]').forEach(button => button.onclick = () => action(button.dataset.action!, button.dataset.id!));
  document.querySelector<HTMLFormElement>('#game-options-form')?.addEventListener('submit', saveOptions);
  document.querySelector<HTMLButtonElement>('#game-options-close')?.addEventListener('click', closeOptions);
  document.querySelector<HTMLButtonElement>('#edit-touch-layout')?.addEventListener('click', openTouchEditor);
  document.querySelector<HTMLButtonElement>('#touch-add')?.addEventListener('click', addTouchButton);
  document.querySelector<HTMLButtonElement>('#touch-close')?.addEventListener('click', closeTouchEditor);
  document.querySelector<HTMLButtonElement>('#touch-cancel')?.addEventListener('click', closeTouchEditor);
  document.querySelector<HTMLFormElement>('#touch-form')?.addEventListener('submit', saveTouchEditor);
  document.querySelector<HTMLInputElement>('#touch-enabled')?.addEventListener('change', event => {
    touchDraftEnabled = (event.target as HTMLInputElement).checked;
  });
  document.querySelectorAll<HTMLButtonElement>('[data-touch-delete]').forEach(button =>
    button.addEventListener('click', () => {
      touchDraft.splice(Number(button.dataset.touchDelete), 1);
      render();
    })
  );
  document.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-touch-field]').forEach(field => {
    field.addEventListener('change', () => {
      const index = Number(field.dataset.index);
      const key = field.dataset.touchField as keyof TouchButton;
      if (!touchDraft[index]) return;
      if (key === 'x' || key === 'y' || key === 'size') {
        (touchDraft[index] as unknown as Record<string, number>)[key] = Number(field.value);
      } else (touchDraft[index] as unknown as Record<string, string>)[key] = field.value;
      render();
    });
  });
  document.querySelectorAll<HTMLButtonElement>('[data-touch-drag]').forEach(button =>
    button.addEventListener('pointerdown', startTouchDrag)
  );
  document.querySelector<HTMLButtonElement>('#choose-icon')?.addEventListener('click', chooseCustomIcon);
  document.querySelector<HTMLButtonElement>('#detect-icon')?.addEventListener('click', restoreAppIcon);
  document.querySelector<HTMLButtonElement>('#game-options-cancel')?.addEventListener('click', closeOptions);
  document.querySelector<HTMLDivElement>('.options-backdrop')?.addEventListener('click', event => {
    if (event.target === event.currentTarget) closeOptions();
  });
  document.querySelector<HTMLSelectElement>('#opt-mode')?.addEventListener('change', event => {
    const windowed = (event.target as HTMLSelectElement).value === 'windowed';
    document.querySelectorAll<HTMLInputElement>('.options-size-field').forEach(field => {
      field.disabled = !windowed;
    });
  });
}
function renderApp(a: InstalledApp): string {
  const local = a.kind === 'local';
  return '<article class="app-card"><div class="app-icon ' + (local ? 'game-icon' : '') +
    '" data-icon-for="' + a.id + '">' + iconMarkup(a) +
    '</div><div class="app-meta"><h3>' + escapeHtml(a.name) + '</h3><p>' + (local ? 'OFFLINE GAME' : 'WEB APPLICATION') +
    '</p><div class="app-url">' + escapeHtml(local ? 'Local HTML5 / WASM package' : (a.url ?? '')) + '</div></div>' +
    '<div class="app-actions"><button class="secondary" data-action="options" data-id="' + a.id + '" title="Adjust Game View, launch window mode, and size">⚙ Game Options</button>' +
    (local || onAndroid ? '' : '<button class="secondary" data-action="capture" data-id="' + a.id + '" title="Open the game online, explore levels, then capture observed same-origin assets">↓ Capture offline</button>') +
    '<button class="primary" data-action="launch" data-id="' + a.id + '">▶ Launch</button>' +
    (onAndroid ? '' : '<button class="secondary" data-action="shortcut" data-id="' + a.id + '" title="Create desktop shortcut">↗ Shortcut</button>') +
    '<button class="danger" data-action="remove" data-id="' + a.id + '" title="Uninstall">×</button></div></article>';
}
function iconMarkup(app: InstalledApp): string {
  if (app.iconData?.startsWith('data:image/png;base64,')) {
    return '<img class="app-image" src="' + escapeHtml(app.iconData) + '" alt="" />';
  }
  return app.kind === 'local' ? '▣' : '◎';
}
function applyIcon(id: string, data: string) {
  const app = apps.find(item => item.id === id);
  if (!app) return;
  app.iconData = data;
  document.querySelectorAll<HTMLElement>('[data-icon-for="' + id + '"]').forEach(element => {
    element.innerHTML = iconMarkup(app);
  });
}
function iconMessage(message: string, error = false) {
  const element = document.querySelector<HTMLElement>('#icon-feedback');
  if (element) {
    element.textContent = message;
    element.className = error ? 'icon-feedback error' : 'icon-feedback';
  }
}
async function setIcon(id: string, command: string, path?: string) {
  if (iconBusy) return;
  iconBusy = true;
  document.querySelectorAll<HTMLButtonElement>('#choose-icon, #detect-icon')
    .forEach(button => button.disabled = true);
  iconMessage('Preparing your app icon…');
  try {
    const data = await invoke<string>(command, path ? { id, path } : { id });
    applyIcon(id, data);
    iconMessage('Icon saved. Reopen the app to update its window icon.');
  } catch (error) {
    iconMessage(errorText(error), true);
  } finally {
    iconBusy = false;
    document.querySelectorAll<HTMLButtonElement>('#choose-icon, #detect-icon')
      .forEach(button => button.disabled = false);
  }
}
async function chooseCustomIcon() {
  const id = editingId;
  if (!id || iconBusy) return;
  try {
    const file = await open({
      directory: false,
      multiple: false,
      filters: [{ name: 'Icon images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'ico', 'gif', 'bmp'] }],
    });
    if (typeof file === 'string') await setIcon(id, 'set_app_icon', file);
  } catch (error) {
    iconMessage('Could not choose icon: ' + errorText(error), true);
  }
}
async function restoreAppIcon() {
  if (editingId && !iconBusy) await setIcon(editingId, 'detect_app_icon');
}
function renderOptions(): string {
  if (!editingId) return '';
  const app = apps.find(item => item.id === editingId);
  if (!app) return '';
  const mode = app.windowMode || 'windowed';
  const modeChoices: [WindowMode, string][] = [
    ['windowed', 'Windowed — resizable'],
    ['maximized', 'Maximized — fill desktop work area'],
    ['fullscreen', 'Fullscreen — borderless display'],
  ];
  const options = modeChoices.map(([value, label]) =>
    '<option value="' + value + '"' + (mode === value ? ' selected' : '') + '>' + escapeHtml(label) + '</option>'
  ).join('');
  return '<div class="options-backdrop"><section class="options-dialog" role="dialog" aria-modal="true" ' +
    'aria-labelledby="options-title">' +
    '<header class="options-heading"><div><div class="eyebrow">WEBCASK RUNTIME</div>' +
    '<h2 id="options-title">Game Options</h2><p>' + escapeHtml(app.name) +
    '</p></div><button id="game-options-close" type="button" class="options-x" aria-label="Close options">×</button></header>' +
    '<form id="game-options-form">' +
    '<div class="options-icon-row"><div class="options-icon-preview" data-icon-for="' +
    app.id + '">' + iconMarkup(app) + '</div><div class="options-icon-controls">' +
    '<label class="options-label">APPLICATION ICON</label>' +
    '<div class="options-icon-buttons"><button class="secondary" id="choose-icon" type="button">Choose Image</button>' +
    '<button class="secondary" id="detect-icon" type="button">' +
    (app.kind === 'online' ? 'Get Website Icon' : 'Get Game Icon') + '</button></div>' +
    '<p id="icon-feedback" class="icon-feedback" aria-live="polite">PNG, JPG, WebP, or ICO. ' +
    'This icon is also used by WebCask and desktop shortcuts.</p></div></div>' +
    '<label class="options-check"><input id="opt-game-view" type="checkbox"' +
    (app.gameFocus ? ' checked' : '') + '><span><strong>Launch in Game View</strong>' +
    '<small>Focus the detected game canvas or iframe, hiding surrounding website elements.</small></span></label>' +
    '<label class="options-label" for="opt-mode">LAUNCH WINDOW MODE</label>' +
    '<select id="opt-mode" class="options-select">' + options + '</select>' +
    '<div class="options-size"><label for="opt-width">WINDOW WIDTH (PX)' +
    '<input class="options-size-field" id="opt-width" type="number" min="480" max="3840" step="1" required value="' +
    (app.windowWidth ?? 1150) + '"' + (mode === 'windowed' ? '' : ' disabled') + '></label>' +
    '<label for="opt-height">WINDOW HEIGHT (PX)' +
    '<input class="options-size-field" id="opt-height" type="number" min="320" max="2160" step="1" required value="' +
    (app.windowHeight ?? 760) + '"' + (mode === 'windowed' ? '' : ' disabled') + '></label></div>' +
    '<div class="touch-options-summary"><div><strong>Touchscreen controls</strong><p>' +
    ((app.touchButtons || []).length || 7) + ' mapped buttons · ' + (app.touchEnabled === false ? 'Disabled' : 'Enabled') +
    '</p></div><button type="button" class="secondary" id="edit-touch-layout">Edit Touch Layout</button></div>' +
    '<p class="options-help">Window size applies when launching windowed. Fullscreen and maximized use the display size. ' +
    'Changes apply on the next launch, including desktop shortcuts.</p>' +
    '<p id="options-error" role="alert" aria-live="polite">' + escapeHtml(optionsError) + '</p>' +
    '<div class="options-footer"><button type="button" id="game-options-cancel" class="secondary">Cancel</button>' +
    '<button type="submit" class="primary" id="game-options-save"' + (optionsSaving ? ' disabled' : '') + '>' +
    (optionsSaving ? 'Saving…' : 'Save Game Options') + '</button></div>' +
    '</form></section></div>';
}
function openOptions(id: string) {
  editingId = id;
  optionsError = '';
  optionsSaving = false;
  iconBusy = false;
  render();
  document.querySelector<HTMLButtonElement>('#choose-icon')?.focus();
}
function closeOptions() {
  if (optionsSaving || iconBusy || touchEditingId) return;
  editingId = null;
  optionsError = '';
  render();
}
async function saveOptions(event: Event) {
  event.preventDefault();
  if (!editingId || optionsSaving) return;
  const id = editingId;
  const gameFocus = document.querySelector<HTMLInputElement>('#opt-game-view')!.checked;
  const windowMode = document.querySelector<HTMLSelectElement>('#opt-mode')!.value as WindowMode;
  const windowWidth = Number(document.querySelector<HTMLInputElement>('#opt-width')!.value);
  const windowHeight = Number(document.querySelector<HTMLInputElement>('#opt-height')!.value);
  if (!Number.isInteger(windowWidth) || !Number.isInteger(windowHeight) ||
      windowWidth < 480 || windowWidth > 3840 || windowHeight < 320 || windowHeight > 2160) {
    optionsError = 'Use a window size between 480 × 320 and 3840 × 2160.';
    document.querySelector<HTMLElement>('#options-error')!.textContent = optionsError;
    return;
  }
  if (!['windowed', 'maximized', 'fullscreen'].includes(windowMode)) {
    optionsError = 'Choose a valid window mode.';
    document.querySelector<HTMLElement>('#options-error')!.textContent = optionsError;
    return;
  }
  optionsSaving = true;
  const button = document.querySelector<HTMLButtonElement>('#game-options-save');
  if (button) { button.disabled = true; button.textContent = 'Saving…'; }
  try {
    await invoke('set_game_options', { id, gameFocus, windowMode, windowWidth, windowHeight });
    editingId = null;
    optionsError = '';
    await refresh();
    notice('Game Options saved. Relaunch to apply.');
  } catch (error) {
    optionsError = errorText(error);
    const label = document.querySelector<HTMLElement>('#options-error');
    if (label) label.textContent = optionsError;
  } finally {
    optionsSaving = false;
    const current = document.querySelector<HTMLButtonElement>('#game-options-save');
    if (current) { current.disabled = false; current.textContent = 'Save Game Options'; }
  }
}


function defaultControls(): TouchButton[] {
  return [
    ['up','▲','ArrowUp',18,63], ['left','◀','ArrowLeft',8,75],
    ['down','▼','ArrowDown',18,85], ['right','▶','ArrowRight',28,75],
    ['a','A','KeyZ',86,75], ['b','B','KeyX',73,85],
    ['start','START','Enter',50,91],
  ].map(([id,label,key,x,y]) => ({
    id: String(id), label: String(label), key: String(key), x: Number(x), y: Number(y), size:56
  }));
}
function openTouchEditor() {
  if (!editingId) return;
  const app = apps.find(item => item.id === editingId);
  if (!app) return;
  touchEditingId = editingId;
  touchDraft = (app.touchButtons || defaultControls()).map(button => ({ ...button }));
  touchDraftEnabled = app.touchEnabled !== false;
  touchSaving = false;
  touchError = '';
  render();
}
function closeTouchEditor() {
  if (touchSaving) return;
  touchEditingId = null;
  touchDraft = [];
  touchError = '';
  render();
}
function addTouchButton() {
  if (touchDraft.length >= 24) { touchError = 'Maximum 24 buttons.'; render(); return; }
  touchNextId++;
  touchDraft.push({
    id:'custom' + Date.now().toString(36) + touchNextId, label:'NEW', key:'Space',
    x:50, y:50, size:56
  });
  render();
}
function startTouchDrag(event: PointerEvent) {
  const button = event.currentTarget as HTMLButtonElement;
  const idx = Number(button.dataset.touchDrag);
  if (!touchDraft[idx]) return;
  const preview = document.querySelector<HTMLElement>('#touch-preview');
  if (!preview) return;
  button.setPointerCapture(event.pointerId);
  const move = (e: PointerEvent) => {
    const area = preview.getBoundingClientRect();
    touchDraft[idx].x = Math.round(Math.max(4, Math.min(96, (e.clientX - area.left) / area.width * 100)));
    touchDraft[idx].y = Math.round(Math.max(4, Math.min(96, (e.clientY - area.top) / area.height * 100)));
    button.style.left = touchDraft[idx].x + '%';
    button.style.top = touchDraft[idx].y + '%';
  };
  const stop = () => {
    button.removeEventListener('pointermove', move);
    button.removeEventListener('pointerup', stop);
    button.removeEventListener('pointercancel', stop);
    render();
  };
  button.addEventListener('pointermove', move);
  button.addEventListener('pointerup', stop, { once:true });
  button.addEventListener('pointercancel', stop, { once:true });
  event.preventDefault();
}
function renderTouchEditor(): string {
  if (!touchEditingId) return '';
  const game = apps.find(app => app.id === touchEditingId);
  if (!game) return '';
  const preview = touchDraft.map((button, index) =>
    '<button type="button" class="touch-preview-button" data-touch-drag="' + index + '" ' +
    'aria-label="Drag ' + escapeHtml(button.label) + ' to change position" ' +
    'style="left:' + button.x + '%;top:' + button.y + '%;width:' +
    button.size + 'px;height:' + button.size + 'px">' + escapeHtml(button.label) + '</button>').join('');
  const rows = touchDraft.map((button, index) => {
    const keyOptions = touchKeys.map(key =>
      '<option value="' + key + '"' + (key === button.key ? ' selected' : '') + '>' +
      key.replace('Key','').replace('Digit','').replace('Space','Spacebar') + '</option>').join('');
    return '<div class="touch-row"><label>LABEL<input aria-label="Button label" ' +
      'data-touch-field="label" data-index="' + index + '" maxlength="16" required value="' +
      escapeHtml(button.label) + '"></label>' +
      '<label>KEY<select data-touch-field="key" data-index="' + index + '">' + keyOptions + '</select></label>' +
      '<label>SIZE (PX)<input type="number" data-touch-field="size" data-index="' + index +
      '" min="32" max="96" step="1" value="' + button.size + '"></label>' +
      '<button class="danger" type="button" data-touch-delete="' + index +
      '" aria-label="Remove button ' + escapeHtml(button.label) + '">×</button></div>';
  }).join('');
  return '<div class="touch-editor-backdrop"><section class="touch-editor" role="dialog" aria-modal="true">' +
    '<div class="touch-editor-head"><div><div class="eyebrow">WEBCASK MOBILE</div>' +
    '<h2>Touch Layout Editor</h2><p>' + escapeHtml(game.name) + '</p></div>' +
    '<button id="touch-close" class="options-x" type="button" aria-label="Close touch editor">×</button></div>' +
    '<form id="touch-form"><label class="options-check"><input id="touch-enabled" type="checkbox"' +
    (touchDraftEnabled ? ' checked' : '') + '><span><strong>Show touchscreen controls</strong>' +
    '<small>Android only. Keyboard mappings are saved per installed app.</small></span></label>' +
    '<p class="touch-help">Drag each button to position it. Change its label, mapped key or size below.</p>' +
    '<div id="touch-preview" class="touch-preview">' + preview + '</div>' +
    '<div class="touch-rows">' + rows + '</div>' +
    '<div class="touch-editor-actions"><button type="button" class="secondary" id="touch-add">＋ Add button</button>' +
    '<span class="touch-counter">' + touchDraft.length + ' / 24</span></div>' +
    '<p class="touch-warning">Compatibility: keyboard events simulated in a web page may not work in games requiring trusted key events or cross-origin iframes.</p>' +
    '<p role="alert" id="touch-error">' + escapeHtml(touchError) + '</p>' +
    '<div class="options-footer"><button type="button" class="secondary" id="touch-cancel">Cancel</button>' +
    '<button type="submit" id="touch-save" class="primary"' + (touchSaving ? ' disabled' : '') +
    '>Save Touch Layout</button></div></form></section></div>';
}
async function saveTouchEditor(event: Event) {
  event.preventDefault();
  if (!touchEditingId || touchSaving) return;
  const id = touchEditingId;
  touchSaving = true;
  try {
    const updated = await invoke<InstalledApp>('set_touch_controls', {
      id, enabled: touchDraftEnabled, buttons: touchDraft,
    });
    const index = apps.findIndex(app => app.id === id);
    if (index >= 0) apps[index] = { ...apps[index], ...updated };
    touchEditingId = null;
    touchError = '';
    render();
    notice('Touch controls saved for this game. Relaunch on Android to apply.');
  } catch (error) {
    touchError = errorText(error);
    const target = document.querySelector('#touch-error');
    if (target) target.textContent = touchError;
  } finally {
    touchSaving = false;
    const button = document.querySelector<HTMLButtonElement>('#touch-save');
    if (button) button.disabled = false;
  }
}

async function pick(folder: boolean) {
  if (busy) return;
  try {
    const result = folder ? await open({ directory: true, multiple: false }) :
      await open({ directory: false, multiple: false, filters: [{ name:'ZIP archives', extensions:['zip'] }] });
    if (typeof result === 'string' && result.trim()) {
      selectedPath = result;
      const label = document.getElementById('path-label');
      if (label) label.textContent = onAndroid ? 'ZIP chosen from Android Files' : result;
      formMessage('info', (folder ? 'Folder' : 'ZIP') + ' selected. Ready to import.');
    } else if (!selectedPath) {
      formMessage('info', 'No game selected. Choose Folder or ZIP.');
    }
  } catch (e) {
    formMessage('error', 'Could not select a game: ' + errorText(e));
  }
}
async function submitInstall(ev: Event) {
  ev.preventDefault();
  if (busy) return;
  const currentMode = mode;
  const name = (document.getElementById('name') as HTMLInputElement).value.trim();
  const address = (document.getElementById('address') as HTMLInputElement | null)?.value.trim();
  draftName = name; draftUrl = address || '';
  if (currentMode === 'game' && !selectedPath) {
    formMessage('error', onAndroid
      ? 'Choose an exported HTML5 ZIP from your phone first.'
      : 'Choose a folder or ZIP containing the exported game files first.');
    return;
  }
  if (!name) {
    formMessage('error', 'Give the game a name before importing.');
    return;
  }
  busy = true;
  const button = document.querySelector<HTMLButtonElement>('#install-button');
  if (button) {
    button.disabled = true;
    button.textContent = currentMode === 'game' ? 'Importing…' : 'Installing…';
  }
  formMessage('info', currentMode === 'game'
    ? 'Checking the game files and importing… This can take a moment for large ZIPs.'
    : 'Installing the web application…');
  try {
    let result: InstalledApp;
    if (currentMode === 'game' && onAndroid) {
      // Android Files returns a content:// URI, not a Rust filesystem path.
      // Use the Android-compatible FS plugin to stage the archive privately.
      // Never pass arbitrary caller-supplied file paths to the import command.
      const token = crypto.randomUUID();
      const relative = 'mobile-imports/' + token + '.zip';
      let staged = false;
      try {
        formMessage('info', 'Copying the ZIP into SitePocket’s private storage…');
        await mkdir('mobile-imports', { baseDir: BaseDirectory.AppLocalData, recursive: true });
        try {
          await copyFile(selectedPath, relative, { toPathBaseDir: BaseDirectory.AppLocalData });
        } catch (error) {
          throw new Error('Android could not read the selected ZIP. Try moving it to Downloads and choose it again. ' +
            errorText(error));
        }
        staged = true;
        formMessage('info', 'Extracting the HTML5 game and checking its index.html…');
        result = await invoke<InstalledApp>('import_android_zip', { name, token });
      } finally {
        // Rust also removes staging on success/failure; this covers an early
        // copy error and does not touch the original Android document.
        if (staged) {
          try { await remove(relative, { baseDir: BaseDirectory.AppLocalData }); } catch (_) {}
        }
      }
    } else {
      result = await invoke<InstalledApp>(
        currentMode === 'website' ? 'install_website' : 'import_game',
        currentMode === 'website' ? { name, url: address } : { name, path: selectedPath }
      );
    }
    selectedPath = ''; draftName = ''; draftUrl = '';
    await refresh();
    formMessage('success', currentMode === 'game'
      ? 'Imported ' + result.name + '. Entry: ' + (result.entryPath || 'index.html') + '. Press Launch below to play.'
      : result.name + ' installed successfully. Ready to launch.');
  } catch (e) {
    // Keep the selected path and form contents so the user can correct the error.
    formMessage('error', errorText(e));
  } finally {
    busy = false;
    const current = document.querySelector<HTMLButtonElement>('#install-button');
    if (current) {
      current.disabled = false;
      current.textContent = currentMode === 'game' ? '＋ Import game' : '＋ Install website';
    }
  }
}
async function action(command: string, id: string) {
  const app = apps.find(a => a.id === id);
  if (!app) return;
  if (command === 'options') { openOptions(id); return; }
  if (command === 'remove' && !window.confirm('Remove ' + app.name + ' and its saved local profile?')) return;
  try {
    if (command === 'capture') {
      await invoke('capture_app', { id });
      notice('Capture window opened. Play through the game, then click Save Offline Candidate. Refresh your library after saving.');
    }
    else if (command === 'launch') { await invoke('launch_app', { id }); notice('Launching ' + app.name + '…'); }
    else if (command === 'shortcut') { const location = await invoke<string>('create_shortcut', { id }); notice('Shortcut created: ' + location); }
    else if (command === 'remove') { await invoke('remove_app', { id }); await refresh(); notice('Application removed.'); }
  } catch (e) {
    if (command === 'launch' && app.kind === 'local' && mode === 'game') {
      formMessage('error', 'Could not launch ' + app.name + ': ' + errorText(e));
    } else {
      notice(errorText(e), true);
    }
  }
}
render();
refresh();

window.addEventListener('focus', () => { if (!busy && !editingId) refresh(); });
window.addEventListener('keydown', event => {
  if (event.key === 'Escape' && touchEditingId && !touchSaving) closeTouchEditor();
  else if (event.key === 'Escape' && editingId && !optionsSaving) closeOptions();
});
