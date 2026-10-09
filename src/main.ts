import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import './style.css';

type InstalledApp = { id: string; name: string; kind: 'online' | 'local'; url: string | null; port: number | null };
type Mode = 'website' | 'game';
let apps: InstalledApp[] = [];
let mode: Mode = 'website';
let selectedPath = '';
let busy = false;

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
    '<p>Turn websites and local HTML5 games into standalone desktop windows.</p></div><div class="version">ALPHA 0.1.0</div></header>' +
    '<section class="install-card"><div class="install-heading"><div class="glyph">' + (mode === 'website' ? '◎' : '▣') + '</div><div>' +
    '<h2>' + (mode === 'website' ? 'Install a website' : 'Import an offline game') + '</h2><p>' +
    (mode === 'website' ? 'Give any website its own dedicated WebCask window.' : 'Bring your HTML, JavaScript or WebAssembly game as a folder or ZIP.') +
    '</p></div></div><form id="install-form"><div class="form-grid"><label>APP NAME<input id="name" placeholder="' +
    (mode === 'website' ? 'e.g. YouTube Music' : 'e.g. My Game') + '" maxlength="80" required /></label>' +
    (mode === 'website' ?
    '<label>WEBSITE ADDRESS<input id="address" type="url" placeholder="https://example.com" required /></label>' :
    '<label>GAME SOURCE<div class="picker"><span id="path-label">' + (selectedPath ? escapeHtml(selectedPath) : 'No folder or ZIP selected') +
    '</span><button type="button" class="secondary" id="pick-folder">Folder</button><button type="button" class="secondary" id="pick-zip">ZIP</button></div></label>') +
    '</div><div class="form-foot"><span>' + (mode === 'website' ? 'Uses the website online. No PWA manifest required.' : 'Runs locally. index.html must be in the selected root.') +
    '</span><button class="primary" type="submit" ' + (busy ? 'disabled' : '') + '>' +
    (busy ? 'Installing…' : mode === 'website' ? '＋ Install website' : '＋ Import game') + '</button></div></form></section>' +
    '<section class="library-head"><div><div class="eyebrow">READY TO LAUNCH</div><h2>Installed applications <span>' + itemCount + '</span></h2></div></section>' +
    '<div class="library">' + (itemCount ? apps.map(renderApp).join('') :
    '<div class="empty"><div class="empty-icon">▦</div><h3>Nothing in your pocket yet</h3><p>Install your first site or import a game to get started.</p></div>') +
    '</div><footer>SitePocket manages your apps · WebCask runs them independently · Windows Alpha</footer></main></div><div id="toast" class="toast"></div>';
  document.querySelectorAll<HTMLButtonElement>('[data-mode]').forEach(button => button.onclick = () => {
    mode = button.dataset.mode as Mode; selectedPath = ''; render();
  });
  document.getElementById('install-form')!.addEventListener('submit', submitInstall);
  document.getElementById('pick-folder')?.addEventListener('click', () => pick(true));
  document.getElementById('pick-zip')?.addEventListener('click', () => pick(false));
  document.querySelectorAll<HTMLButtonElement>('[data-action]').forEach(button => button.onclick = () => action(button.dataset.action!, button.dataset.id!));
}
function renderApp(a: InstalledApp): string {
  const local = a.kind === 'local';
  return '<article class="app-card"><div class="app-icon ' + (local ? 'game-icon' : '') + '">' + (local ? '▣' : '◎') +
    '</div><div class="app-meta"><h3>' + escapeHtml(a.name) + '</h3><p>' + (local ? 'OFFLINE GAME' : 'WEB APPLICATION') +
    '</p><div class="app-url">' + escapeHtml(local ? 'Local HTML5 / WASM package' : (a.url ?? '')) + '</div></div>' +
    '<div class="app-actions"><button class="primary" data-action="launch" data-id="' + a.id + '">▶ Launch</button>' +
    '<button class="secondary" data-action="shortcut" data-id="' + a.id + '" title="Create desktop shortcut">↗ Shortcut</button>' +
    '<button class="danger" data-action="remove" data-id="' + a.id + '" title="Uninstall">×</button></div></article>';
}
async function pick(folder: boolean) {
  try {
    const result = folder ? await open({ directory: true, multiple: false }) :
      await open({ directory: false, multiple: false, filters: [{ name:'ZIP archives', extensions:['zip'] }] });
    if (typeof result === 'string') { selectedPath = result; document.getElementById('path-label')!.textContent = result; }
  } catch (e) { notice(errorText(e), true); }
}
async function submitInstall(ev: Event) {
  ev.preventDefault();
  if (busy) return;
  const name = (document.getElementById('name') as HTMLInputElement).value.trim();
  const address = (document.getElementById('address') as HTMLInputElement | null)?.value.trim();
  if (mode === 'game' && !selectedPath) { notice('Choose a folder or ZIP first.', true); return; }
  busy = true;
  const currentMode = mode;
  try {
    await invoke(currentMode === 'website' ? 'install_website' : 'import_game',
      currentMode === 'website' ? { name, url: address } : { name, path: selectedPath });
    selectedPath = '';
    await refresh();
    notice(currentMode === 'website' ? 'Website installed.' : 'Offline game imported.');
  } catch (e) { notice(errorText(e), true); }
  finally { busy = false; render(); }
}
async function action(command: string, id: string) {
  const app = apps.find(a => a.id === id);
  if (!app) return;
  if (command === 'remove' && !window.confirm('Remove ' + app.name + ' and its saved local profile?')) return;
  try {
    if (command === 'launch') { await invoke('launch_app', { id }); notice('Launching ' + app.name + '…'); }
    else if (command === 'shortcut') { const location = await invoke<string>('create_shortcut', { id }); notice('Shortcut created: ' + location); }
    else if (command === 'remove') { await invoke('remove_app', { id }); await refresh(); notice('Application removed.'); }
  } catch (e) { notice(errorText(e), true); }
}
render();
refresh();
