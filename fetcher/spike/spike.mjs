#!/usr/bin/env node
// THROWAWAY spike harness for finima-fetch iteration 0 (spec §3.1).
// Deleted by iteration 1's first task. Prints no page text, query strings
// or file contents -- only URL origin+path, truncated titles, counts and
// byte sniffs. The human performs every login and click on the bank site.
import { execFileSync, spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, readSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import puppeteer from 'puppeteer-core';

const HOME = process.env.SPIKE_HOME ?? join(homedir(), 'Library', 'Application Support', 'finima-fetch-spike');
const PROFILE = join(HOME, 'profile');
const STAGING = join(HOME, 'staging');
const STATE = join(HOME, 'session.json');
const DEFAULT_BRAVE = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser';
const BOFA_URL = 'https://www.bankofamerica.com/';
const BOFA_HOST = /(^|\.)bankofamerica\.com$/;
const PUBLIC_DOWNLOAD_URL = 'https://github.com/pacphi/finima/archive/refs/heads/main.zip';
const INTERACTIVE = new Set(['link', 'button', 'textbox', 'searchbox', 'combobox', 'listbox', 'option', 'checkbox', 'radio', 'menuitem', 'tab', 'switch', 'spinbutton']);

const { positionals, values: opts } = parseArgs({
  allowPositionals: true,
  options: {
    mode: { type: 'string', default: 'minimal' },
    'browser-path': { type: 'string', default: DEFAULT_BRAVE },
    minutes: { type: 'string', default: '20' },
    interval: { type: 'string', default: '30' },
    'trace-requests': { type: 'boolean', default: false },
  },
});

const log = (obj) => console.log(JSON.stringify({ ts: new Date().toISOString(), ...obj }));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const isBofa = (u) => {
  try {
    return BOFA_HOST.test(new URL(u).hostname);
  } catch {
    return false;
  }
};
const safeUrl = (u) => {
  try {
    const x = new URL(u);
    if (x.protocol === 'blob:') return `blob:${new URL(x.pathname).origin}`;
    return `${x.origin}${x.pathname}`;
  } catch {
    return '(unparseable)';
  }
};

function readState() {
  if (!existsSync(STATE)) throw new Error('No spike session. Run: node fetcher/spike/spike.mjs start');
  return JSON.parse(readFileSync(STATE, 'utf8'));
}

// minimal = attach only to Bank of America page targets, Network/Issues domains off.
// maximal = puppeteer defaults (attach to every tab, Network + Issues on).
async function connect(mode) {
  const { port, wsPath } = readState();
  const browserWSEndpoint = `ws://127.0.0.1:${port}${wsPath}`;
  if (mode === 'maximal') return puppeteer.connect({ browserWSEndpoint, defaultViewport: null });
  if (mode !== 'minimal') throw new Error(`--mode must be minimal or maximal, got ${mode}`);
  return puppeteer.connect({
    browserWSEndpoint,
    defaultViewport: null,
    networkEnabled: false,
    issuesEnabled: false,
    targetFilter: (t) => t.type() !== 'page' || isBofa(t.url()),
  });
}

async function bofaPage(browser) {
  const page = (await browser.pages()).find((p) => isBofa(p.url()));
  if (!page) throw new Error('No Bank of America tab is open in the spike browser.');
  return page;
}

function sniff(file) {
  const fd = openSync(file, 'r');
  const buf = Buffer.alloc(64);
  const n = readSync(fd, buf, 0, 64, 0);
  closeSync(fd);
  const head = buf.subarray(0, n).toString('latin1').trimStart();
  const firstLine = head.split(/\r?\n/)[0];
  let kind = 'unknown';
  if (/^OFXHEADER:/.test(head) || /<OFX>/i.test(head)) kind = 'ofx';
  else if (/^<\?xml/i.test(head)) kind = 'xml';
  else if (/^<!doctype html|^<html/i.test(head)) kind = 'HTML (bad export)';
  else if (head.startsWith('!Type:')) kind = 'qif';
  else if (head.startsWith('PK')) kind = 'zip/xlsx';
  else if (/[,;\t]/.test(firstLine)) kind = 'csv-like';
  return { kind, firstLineLength: firstLine.length };
}

async function traceRequests(page) {
  const s = await page.createCDPSession();
  const byUrl = new Map();
  s.on('Network.requestWillBeSent', (e) => byUrl.set(e.request.url, { method: e.request.method, type: e.type }));
  s.on('Network.responseReceived', (e) => {
    const h = Object.fromEntries(Object.entries(e.response.headers).map(([k, v]) => [k.toLowerCase(), v]));
    byUrl.set(e.response.url, { ...(byUrl.get(e.response.url) ?? {}), status: e.response.status, mimeType: e.response.mimeType, contentDisposition: 'content-disposition' in h });
  });
  await s.send('Network.enable');
  return { match: (url) => (url ? (byUrl.get(url) ?? null) : null), stop: () => s.detach().catch(() => {}) };
}

async function start() {
  const exe = opts['browser-path'];
  if (!existsSync(exe)) throw new Error(`Browser not found at ${exe}; pass --browser-path`);
  if (existsSync(STATE)) throw new Error(`State exists at ${STATE}; run "end" first (delete it if the browser is already gone).`);
  mkdirSync(PROFILE, { recursive: true, mode: 0o700 });
  mkdirSync(STAGING, { recursive: true, mode: 0o700 });
  const portFile = join(PROFILE, 'DevToolsActivePort');
  rmSync(portFile, { force: true });
  const t0 = Date.now();
  const child = spawn(exe, [`--user-data-dir=${PROFILE}`, '--remote-debugging-port=0', '--no-first-run', '--no-default-browser-check', BOFA_URL], { detached: true, stdio: 'ignore' });
  child.unref();
  let content = '';
  while (Date.now() - t0 < 15000) {
    if (existsSync(portFile)) {
      content = readFileSync(portFile, 'utf8');
      if (content.includes('\n')) break;
    }
    await sleep(100);
  }
  if (!content.includes('\n')) throw new Error(`DevToolsActivePort did not appear within 15s (pid ${child.pid}). Check whether Brave handed off to an already-running instance.`);
  const [portLine, wsPath] = content.trim().split('\n');
  const state = { pid: child.pid, port: Number(portLine), wsPath, exe, profileDir: PROFILE, startedAt: new Date().toISOString() };
  writeFileSync(STATE, JSON.stringify(state, null, 2), { mode: 0o600 });
  let listen = '(lsof unavailable)';
  try {
    listen = execFileSync('lsof', ['-nP', `-iTCP:${state.port}`, '-sTCP:LISTEN'], { encoding: 'utf8' }).trim();
  } catch {
    /* keep default */
  }
  log({ cmd: 'start', pid: child.pid, port: state.port, devToolsActivePortMs: Date.now() - t0, listen });
}

async function probe() {
  const browser = await connect(opts.mode);
  try {
    const page = await bofaPage(browser);
    const signals = await page.evaluate(() => ({
      webdriver: navigator.webdriver,
      chromeObject: typeof window.chrome === 'object',
      headlessUA: navigator.userAgent.includes('Headless'),
      plugins: navigator.plugins.length,
      languages: navigator.languages.length,
    }));
    log({ cmd: 'probe', mode: opts.mode, version: await browser.version(), url: safeUrl(page.url()), ...signals });
  } finally {
    await browser.disconnect();
  }
}

async function hold() {
  const browser = await connect(opts.mode);
  const minutes = Number(opts.minutes);
  process.on('SIGINT', async () => {
    await browser.disconnect();
    log({ cmd: 'hold', event: 'detached' });
    process.exit(0);
  });
  log({ cmd: 'hold', mode: opts.mode, minutes, note: 'Attached. Do the manual flow now; Ctrl+C to detach early.' });
  const stopAt = Date.now() + minutes * 60000;
  while (Date.now() < stopAt) {
    const pages = await browser.pages(); // in maximal mode this attaches to every tab
    log({ cmd: 'hold', tabs: pages.map((p) => safeUrl(p.url())) });
    await sleep(30000);
  }
  await browser.disconnect();
}

async function reload() {
  const browser = await connect(opts.mode);
  try {
    const page = await bofaPage(browser);
    const before = safeUrl(page.url());
    await page.reload({ waitUntil: 'domcontentloaded', timeout: 60000 });
    log({ cmd: 'reload', mode: opts.mode, before, after: safeUrl(page.url()) });
  } finally {
    await browser.disconnect();
  }
}

async function armDownload() {
  const browser = await connect(opts.mode);
  const session = await browser.target().createCDPSession();
  const seen = new Map();
  let trace = null;
  try {
    await session.send('Browser.setDownloadBehavior', { behavior: 'allowAndName', downloadPath: STAGING, eventsEnabled: true });
    if (opts['trace-requests']) trace = await traceRequests(await bofaPage(browser));
    const done = new Promise((resolve) => {
      session.on('Browser.downloadWillBegin', (e) => {
        seen.set(e.guid, e);
        log({ event: 'downloadWillBegin', url: safeUrl(e.url), scheme: e.url.split(':')[0], suggestedFilename: e.suggestedFilename });
      });
      session.on('Browser.downloadProgress', (e) => {
        if (e.state !== 'inProgress') resolve(e);
      });
    });
    log({ cmd: 'arm-download', mode: opts.mode, staging: STAGING, note: 'Armed. Click the bank export now.' });
    const result = await Promise.race([done, sleep(Number(opts.minutes) * 60000).then(() => null)]);
    if (!result) {
      log({ event: 'timeout', note: 'No download completed. Record what the site did (new window? in-page file? error?).' });
      return;
    }
    const begin = seen.get(result.guid);
    const file = join(STAGING, result.guid);
    const present = existsSync(file);
    log({
      event: 'downloadProgress',
      state: result.state,
      bytes: present ? statSync(file).size : 0,
      sniff: present ? sniff(file) : null,
      suggestedFilename: begin?.suggestedFilename,
      request: trace?.match(begin?.url) ?? null,
      stagedAs: present ? file : null,
    });
  } finally {
    await session.send('Browser.setDownloadBehavior', { behavior: 'default' }).catch(() => {});
    await session.detach().catch(() => {});
    await trace?.stop();
    await browser.disconnect();
  }
}

async function axSize() {
  const browser = await connect(opts.mode);
  try {
    const page = await bofaPage(browser);
    const s = await page.createCDPSession();
    const t0 = Date.now();
    const { nodes } = await s.send('Accessibility.getFullAXTree');
    const ms = Date.now() - t0;
    const live = nodes.filter((n) => !n.ignored);
    const interactive = live.filter((n) => INTERACTIVE.has(n.role?.value));
    log({ cmd: 'ax-size', url: safeUrl(page.url()), ms, total: nodes.length, notIgnored: live.length, interactive: interactive.length, frames: page.frames().length });
    await s.detach();
  } finally {
    await browser.disconnect();
  }
}

// Is Browser.setDownloadBehavior browser-wide? Arm it, open a non-bank tab that
// downloads a public zip, and see whether the event fires for that tab too.
async function dlScope() {
  const browser = await connect(opts.mode);
  const session = await browser.target().createCDPSession();
  let targetId = null;
  try {
    await session.send('Browser.setDownloadBehavior', { behavior: 'allowAndName', downloadPath: STAGING, eventsEnabled: true });
    const fired = new Promise((resolve) => session.on('Browser.downloadWillBegin', (e) => resolve(e)));
    ({ targetId } = await session.send('Target.createTarget', { url: PUBLIC_DOWNLOAD_URL }));
    const e = await Promise.race([fired, sleep(60000).then(() => null)]);
    log({ cmd: 'dl-scope', affectsOtherTabs: !!e, url: e ? safeUrl(e.url) : null, cleanup: e ? join(STAGING, e.guid) : null });
  } finally {
    if (targetId) await session.send('Target.closeTarget', { targetId }).catch(() => {});
    await session.send('Browser.setDownloadBehavior', { behavior: 'default' }).catch(() => {});
    await session.detach().catch(() => {});
    await browser.disconnect();
  }
}

async function idleWatch() {
  const browser = await connect('minimal');
  const page = await bofaPage(browser);
  const minutes = Number(opts.minutes);
  const intervalMs = Number(opts.interval) * 1000;
  const t0 = Date.now();
  let last = '';
  log({ cmd: 'idle-watch', minutes, intervalSec: intervalMs / 1000, note: 'Do not touch the Bank of America tab until this finishes.' });
  while (Date.now() - t0 < minutes * 60000) {
    const snap = await page
      .evaluate(() => ({
        title: document.title.slice(0, 80),
        dialogs: document.querySelectorAll('[role=dialog],[role=alertdialog],dialog[open]').length,
      }))
      .catch(() => ({ title: '(navigating)', dialogs: -1 }));
    const cur = { url: safeUrl(page.url()), ...snap };
    const key = JSON.stringify(cur);
    if (key !== last) {
      log({ event: 'change', minute: Number(((Date.now() - t0) / 60000).toFixed(1)), ...cur });
      last = key;
    }
    await sleep(intervalMs);
  }
  await browser.disconnect();
}

async function end() {
  const browser = await connect('maximal').catch(() => null);
  if (browser) await browser.close();
  rmSync(STATE, { force: true });
  log({ cmd: 'end', closed: !!browser });
}

const commands = { start, probe, hold, reload, 'arm-download': armDownload, 'ax-size': axSize, 'dl-scope': dlScope, 'idle-watch': idleWatch, end };
const run = commands[positionals[0]];
if (!run) {
  console.error(`usage: node fetcher/spike/spike.mjs <${Object.keys(commands).join('|')}> [--mode minimal|maximal] [--minutes N] [--interval S] [--trace-requests] [--browser-path P]`);
  process.exit(2);
}
run().catch((err) => {
  log({ error: err.message });
  process.exit(1);
});
