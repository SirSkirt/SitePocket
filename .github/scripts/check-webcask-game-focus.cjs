// Shared Game View regression checks for BOTH Windows Wry and Android Tauri.
// No browser, IPC/native privileges, third-party packages or network needed.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const script = fs.readFileSync('crates/webcask-runtime/src/game_focus.js', 'utf8');

function browser() {
  const intervals = [];
  let timeouts = [];
  const observers = [];
  const events = new Map();

  class Element {
    constructor(tag) {
      this.tagName = tag.toUpperCase();
      this.children = [];
      this.parentElement = null;
      this.attributes = {};
      this.className = '';
      this.textContent = '';
      this.rect = { width: 0, height: 0 };
      this.style = { cssText: '', setProperty(k, v) { this[k] = v; } };
      this.listeners = {};
    }
    setAttribute(k, v) { this.attributes[k] = String(v); }
    getAttribute(k) { return this.attributes[k] ?? null; }
    removeAttribute(k) { delete this.attributes[k]; }
    appendChild(el) {
      el.parentElement = this; this.children.push(el);
      for (const observer of observers) observer();
      return el;
    }
    removeChild(el) {
      this.children = this.children.filter(x => x !== el);
      el.parentElement = null;
      for (const observer of observers) observer();
    }
    contains(el) {
      return this === el || this.children.some(child => child.contains(el));
    }
    matches(selectors) {
      return selectors.split(',').some(selector => {
        const s = selector.trim();
        if (s.startsWith('#')) return this.getAttribute('id') === s.slice(1);
        if (s.startsWith('.')) return (this.className || '').split(/\s+/).includes(s.slice(1));
        if (s === '[data-game-root]') return this.getAttribute('data-game-root') != null;
        if (s === '[data-webcask-overlay]') return this.getAttribute('data-webcask-overlay') != null;
        return s.toLowerCase() === this.tagName.toLowerCase();
      });
    }
    closest(selector) {
      let e = this;
      while (e) { if (e.matches(selector)) return e; e = e.parentElement; }
      return null;
    }
    getBoundingClientRect() { return this.rect; }
    addEventListener(name, fn) { (this.listeners[name] ||= []).push(fn); }
    removeEventListener(name, fn) {
      this.listeners[name] = (this.listeners[name] || []).filter(x => x !== fn);
    }
    dispatch(name, arg = {}) {
      for (const fn of this.listeners[name] || []) fn(arg);
    }
  }

  const html = new Element('html');
  const head = new Element('head');
  const body = new Element('body');
  html.appendChild(head); html.appendChild(body);
  const docEvents = {};
  const document = {
    documentElement: html, head, body, readyState: 'complete',
    contains: el => html.contains(el),
    createElement: tag => new Element(tag),
    querySelectorAll(selectors) {
      const matches = [];
      function visit(node) {
        if (node.matches(selectors)) matches.push(node);
        node.children.forEach(visit);
      }
      visit(html);
      return matches;
    },
    getElementById(id) {
      return document.querySelectorAll('#' + id)[0] || null;
    },
    addEventListener(name, fn) { (docEvents[name] ||= []).push(fn); },
    elementFromPoint() { return document.querySelectorAll('canvas')[0] || null; }
  };
  class Observer {
    constructor(fn) { this.fn = fn; }
    observe() { observers.push(() => this.fn()); }
  }
  const window = {
    innerWidth: 940, innerHeight: 600, MutationObserver: Observer,
    visualViewport: null, ResizeObserver: undefined,
    setTimeout: fn => { timeouts.push(fn); return timeouts.length; },
    setInterval: fn => { intervals.push(fn); return intervals.length; },
    requestAnimationFrame: fn => { fn(); },
    getComputedStyle: el => ({
      display: el.getAttribute('data-hidden') ? 'none' : 'block',
      visibility: 'visible'
    }),
    addEventListener(name, fn) { (events.get(name) || events.set(name, []).get(name)).push(fn); },
    removeEventListener(name, fn) { events.set(name, (events.get(name) || []).filter(x => x !== fn)); }
  };
  window.top = window;
  function run() {
    vm.runInNewContext(script, { window, document, HTMLElement: Element, MutationObserver: Observer }, {
      timeout: 1200, filename: 'game_focus.js'
    });
  }
  function tick() {
    intervals.slice().forEach(fn => fn());
    const ready = timeouts; timeouts = [];
    ready.forEach(fn => fn());
  }
  function element(tag, width, height, parent = body, id = '') {
    const el = new Element(tag);
    el.rect = { width, height };
    if (id) el.setAttribute('id', id);
    parent.appendChild(el);
    return el;
  }
  function badge() { return document.querySelectorAll('[data-webcask-overlay]')[0]; }
  return { window, document, body, html, element, run, tick, badge };
}

// A canvas already mounted when document scripts execute is focused.
{
  const b = browser();
  const canvas = b.element('canvas', 800, 450);
  canvas.setAttribute('style', 'border:1px solid red');
  b.run();
  assert.equal(b.html.getAttribute('data-webcask-game'), 'on');
  assert.equal(canvas.getAttribute('data-webcask-target'), 'true');
  assert.ok(b.badge(), 'manual Toggle and Pick remain available');
  assert.equal(b.badge().children.filter(x => x.tagName === 'BUTTON').length, 2);
  const toggle = b.badge().children.find(x => x.textContent === 'Toggle');
  toggle.dispatch('click');
  assert.equal(b.html.getAttribute('data-webcask-game'), null);
  assert.equal(canvas.getAttribute('style'), 'border:1px solid red');
  toggle.dispatch('click');
  assert.equal(b.html.getAttribute('data-webcask-game'), 'on');
}

// Delayed canvas creation must be found long after the old 30-attempt cap.
{
  const b = browser();
  b.run();
  assert.match(b.badge().children[0].textContent, /Waiting for game canvas/);
  for (let i = 0; i < 42; i++) b.tick();
  const canvas = b.element('canvas', 480, 300);
  b.tick();
  assert.equal(canvas.getAttribute('data-webcask-target'), 'true');
}

// A game that replaces its canvas should recover without a reload.
{
  const b = browser();
  const oldCanvas = b.element('canvas', 640, 360);
  b.run();
  b.body.removeChild(oldCanvas);
  const newCanvas = b.element('canvas', 640, 360);
  b.tick();
  assert.equal(newCanvas.getAttribute('data-webcask-target'), 'true');
  assert.equal(b.html.getAttribute('data-webcask-game'), 'on');
}

// Keep a named game wrapper with sibling HUD/menus in Game View.
{
  const b = browser();
  const root = b.element('div', 810, 510, b.body, 'game-root');
  const canvas = b.element('canvas', 800, 450, root);
  b.element('div', 800, 70, root, 'game-hud');
  b.run();
  assert.equal(root.getAttribute('data-webcask-target'), 'true');
  assert.equal(canvas.getAttribute('data-webcask-target'), null);
}

// Canvas-less game roots and smaller Android split-screen canvases work.
{
  const b = browser();
  const root = b.element('div', 340, 260, b.body, 'game-container');
  b.run();
  assert.equal(root.getAttribute('data-webcask-target'), 'true');
}
{
  const b = browser();
  const canvas = b.element('canvas', 180, 110);
  b.run();
  assert.equal(canvas.getAttribute('data-webcask-target'), 'true');
}

console.log('Shared Windows/Android WebCask Game View regression checks passed.');
