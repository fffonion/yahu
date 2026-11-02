// Deployed assets with isolated, read-only browser fixtures; no real chat traffic.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { mkdir, readFile } from 'node:fs/promises';
const base = process.env.YAHU_BASE_URL || 'http://127.0.0.1:9642';
const out = process.env.YAHU_QA_OUT;
if (out) await mkdir(out, { recursive: true });
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const root = '/srv/workspace';
const dir = path => ({ name: path.split('/').pop(), path, kind: 'dir' });
const file = path => ({ name: path.split('/').pop(), path, kind: 'file', size: 32 });
const names = ['read.ts', 'write.ts', 'patch.ts', 'slow.ts', 'fast.ts', 'search.ts'];
const tree = {
  '': [dir('project')],
  project: [dir('project/src')],
  'project/src': [...Array.from({ length: 60 }, (_, i) => file(`project/src/example-${i}.ts`)), ...names.map(name => file('project/src/' + name))],
};
const tool = (id, name, path, result = {}) => ({ id, role: 'tool', content: JSON.stringify(result), tool_name: name, tool_input: { path }, timestamp: 1800000000 });
const messages = [
  { id: '1', role: 'user', content: 'Inspect the example files.', timestamp: 1800000000 },
  tool('2', 'read_file', root + '/project/src/read.ts', { content: '1|export const value = 1;' }),
  tool('3', 'write_file', 'project/src/write.ts'),
  tool('4', 'patch', './project/src/patch.ts'),
  tool('5', 'read_file', 'project/src/slow.ts'),
  tool('6', 'read_file', 'project/src/fast.ts'),
  tool('7', 'search_files', 'project/src', { total_count: 1, matches_text: root + '/project/src/search.ts\n  1: export const result = true;' }),
  tool('8', 'search_files', 'project/src', { total_count: 1, files: ['project/src/write.ts'] }),
  tool('9', 'read_file', '/srv/outside/private.ts'),
  tool('10', 'read_file', 'project/src/missing.ts'),
  { id: '11', role: 'assistant', content: 'Example inspection complete.', timestamp: 1800000010 },
];
const session = { id: 'workspace-path-fixture', title: 'Example file tools', source: 'api', model: 'example', message_count: messages.length, started_at: 1800000000, last_active: 1800000010 };
const assets = html => [...html.matchAll(/(?:src|href)="([^"]+)"/g)].map(x => x[1]).filter(x => x.startsWith('/assets/')).sort();
let browser, stage = 'login';
try {
  const env = await readFile(process.env.YAHU_ENV_FILE || '/etc/yahu.env', 'utf8');
  const raw = env.match(/^HERMES_WEBUI_AUTH_KEY=(.*)$/m)?.[1]?.trim();
  assert(raw, 'missing login key');
  const key = /^(['"]).*\1$/.test(raw) ? raw.slice(1, -1) : raw;
  const login = await fetch(base + '/login', { method: 'POST', redirect: 'manual', body: new URLSearchParams({ password: key }) });
  const cookie = login.headers.get('set-cookie')?.split(';')[0];
  assert(login.status === 303 && cookie, 'login failed');
  const split = cookie.indexOf('=');
  const expectedAssets = assets(await readFile(new URL('../dist/index.html', import.meta.url), 'utf8'));
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_EXECUTABLE || undefined, args: ['--no-sandbox'] });
  for (const [name, width, compact] of [['desktop', 1440, false], ['compact', 1440, true], ['tablet', 1024, true], ['mobile', 390, true]]) {
    stage = name;
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: width === 390, hasTouch: width <= 1024, serviceWorkers: 'block' });
    await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: base, httpOnly: true, sameSite: 'Lax' }]);
    await context.addInitScript(compact => {
      localStorage.setItem('lang', 'en');
      localStorage.setItem('desktopCompactMessages', compact ? '1' : '0');
      class EmptyEvents { close() {} addEventListener() {} removeEventListener() {} }
      window.EventSource = EmptyEvents;
      window.WebSocket = EmptyEvents;
    }, compact);
    const page = await context.newPage();
    const requests = [];
    let errors = 0, writes = 0, slowStarted = 0;
    page.on('pageerror', () => errors++);
    await page.route('**/*', async route => {
      const request = route.request(), url = new URL(request.url()), p = url.pathname;
      if (url.origin !== base) return route.abort();
      if (request.method() !== 'GET') { writes++; return route.abort(); }
      if (p === '/' || p.startsWith('/assets/') || p.startsWith('/icon') || p === '/manifest.json') return route.continue();
      let body = {};
      if (p === '/workspace/list') {
        const path = url.searchParams.get('path') || '';
        requests.push(['list', path]);
        return route.fulfill({ status: tree[path] ? 200 : 404, contentType: 'application/json', body: JSON.stringify({ root, path, entries: tree[path] || [] }) });
      }
      if (p === '/workspace/file') {
        const path = url.searchParams.get('path');
        requests.push(['file', path]);
        if (path.endsWith('/slow.ts')) { slowStarted++; await new Promise(resolve => setTimeout(resolve, 450)); }
        return route.fulfill({ status: 200, contentType: 'text/plain', body: `// Preview: ${path}\nexport const example = true;` });
      }
      if (p === '/sessions/search') body = { data: [session] };
      if (p.startsWith('/chat/messages/')) body = { data: messages, total: messages.length, has_older: false, has_newer: false };
      if (p.startsWith('/chat/user-nav/')) body = { data: [], total: messages.length };
      if (p.endsWith('/canonical')) body = { id: session.id, canonical_id: session.id };
      if (p.startsWith('/hermes/api/sessions/')) body = { data: session };
      if (p.endsWith('/status')) body = { running: false };
      if (p.endsWith('/reasoning')) body = { reasoning_effort: 'medium' };
      if (p === '/models-cache') body = { providers: [{ slug: 'example', name: 'Example', models: ['example'] }] };
      if (p.startsWith('/skills/')) body = { data: [] };
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    });
    await page.goto(base + '/?workspace-path-qa=1#/chat/' + session.id);
    await page.locator('.turn-detail-summary').first().click();
    const summary = id => page.locator(`[data-message-id="${id}"] .tool-summary`);
    await summary('2').waitFor();
    await page.waitForTimeout(350);
    const before = requests.length;
    await summary('2').click();
    await page.waitForTimeout(100);
    assert(requests.length === before, 'collapsed workspace performed navigation');
    const originalHash = await page.evaluate(() => location.hash);
    if (width > 900) {
      await page.locator('.workspace-rail-btn').click();
      const checkSelected = async path => {
        await page.waitForFunction(path => document.querySelector('.workspace-tree-row.selected')?.getAttribute('data-workspace-path') === path, path);
        if (!path.endsWith('/src')) await page.waitForFunction(path => document.querySelector('.workspace .preview')?.textContent.includes('Preview: ' + path), path);
        const geometry = await page.locator('.workspace-tree-row.selected').evaluate(row => { const tree = row.closest('.workspace-tree'); const r = row.getBoundingClientRect(), t = tree.getBoundingClientRect(); return { visible: r.top >= t.top - 1 && r.bottom <= t.bottom + 1, rowTop: r.top, rowBottom: r.bottom, treeTop: t.top, treeBottom: t.bottom, scrollTop: tree.scrollTop }; });
        if (!geometry.visible) console.log(JSON.stringify({ selectedPath: path, geometry }));
        assert(geometry.visible, 'selected path is outside tree viewport');
        assert(await page.evaluate(() => location.hash) === originalHash, 'chat route changed');
      };
      for (const [id, path] of [['2', 'project/src/read.ts'], ['3', 'project/src/write.ts'], ['4', 'project/src/patch.ts'], ['7', 'project/src']]) {
        const start = requests.length;
        await summary(id).click();
        await checkSelected(path);
        const queried = requests.slice(start).filter(([kind]) => kind === 'list').map(([, path]) => path);
        assert(queried.every(path => ['', 'project', 'project/src'].includes(path)), 'navigation scanned unrelated directories');
      }
      await page.locator('[data-message-id="7"] .search-files-path').click();
      await checkSelected('project/src/search.ts');
      await summary('8').click();
      await page.locator('[data-message-id="8"] .search-files-file').click();
      await checkSelected('project/src/write.ts');
      await summary('5').click();
      while (!slowStarted) await page.waitForTimeout(10);
      await summary('6').click();
      await checkSelected('project/src/fast.ts');
      await page.waitForTimeout(550);
      await checkSelected('project/src/fast.ts');
      const nextSlow = slowStarted + 1;
      await summary('5').click();
      while (slowStarted < nextSlow) await page.waitForTimeout(10);
      const manualFile = page.locator('.workspace-tree-row[data-workspace-path="project/src/fast.ts"]');
      await manualFile.focus();
      await page.keyboard.press('Enter');
      await checkSelected('project/src/fast.ts');
      await page.waitForTimeout(550);
      await checkSelected('project/src/fast.ts');
      const previous = requests.length;
      await summary('9').click();
      await page.waitForTimeout(100);
      assert(requests.slice(previous).every(([kind, path]) => kind === 'list' && path === ''), 'outside path reached file transport');
      await checkSelected('project/src/fast.ts');
      await summary('10').click();
      await page.waitForTimeout(100);
      await checkSelected('project/src/fast.ts');
      if (out) await page.locator('.workspace').screenshot({ path: `${out}/workspace-tool-${name}.png` });
      await page.locator('.workspace-head button').click();
      const collapsed = requests.length;
      await summary('3').click();
      await page.waitForTimeout(100);
      assert(requests.length === collapsed, 're-collapsed workspace performed navigation');
    }
    assert(errors === 0 && writes === 0, 'browser error or non-read request');
    assert(JSON.stringify(assets(await page.content())) === JSON.stringify(expectedAssets), 'deployed asset mismatch');
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), 'horizontal overflow');
    console.log(JSON.stringify({ viewport: name, navigation: width > 900, collapsedNoNavigation: true, unchangedChatRoute: true, errors, writes, deployedAssets: true }));
    await context.close();
  }
} catch (error) {
  console.error(JSON.stringify({ failedStage: stage, errorClass: error.constructor.name, assertion: error.constructor === Error ? error.message : undefined }));
  process.exitCode = 1;
} finally { await browser?.close(); }
