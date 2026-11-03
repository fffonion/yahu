// Deployed UI verification with isolated multi-account usage fixtures.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { readFile, mkdir } from 'node:fs/promises';
const base = process.env.YAHU_BASE_URL || 'http://127.0.0.1:9642';
const out = process.env.YAHU_QA_OUT;
if (out) await mkdir(out, { recursive: true });
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const assets = html => [...html.matchAll(/(?:src|href)="([^"]+)"/g)].map(x => x[1]).filter(x => x.startsWith('/assets/')).sort();
let browser, stage = 'login';
try {
  const env = await readFile(process.env.YAHU_ENV_FILE || '/etc/yahu.env', 'utf8');
  const raw = env.match(/^HERMES_WEBUI_AUTH_KEY=(.*)$/m)?.[1]?.trim();
  assert(raw, 'login key unavailable');
  const key = /^(['"]).*\1$/.test(raw) ? raw.slice(1, -1) : raw;
  const login = await fetch(base + '/login', { method: 'POST', redirect: 'manual', body: new URLSearchParams({ password: key }) });
  const cookie = login.headers.get('set-cookie')?.split(';')[0];
  assert(login.status === 303 && cookie, 'login failed');
  const split = cookie.indexOf('=');
  const expectedAssets = assets(await readFile(new URL('../dist/index.html', import.meta.url), 'utf8'));
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE || undefined, headless: true, args: ['--no-sandbox'] });
  for (const width of [1440, 390]) for (const [count, enabled] of [[2, true], [3, true], [4, true], [4, false]]) {
    stage = `${width}-${count}-${enabled}`;
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: width === 390, hasTouch: width === 390, serviceWorkers: 'block' });
    await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: base, httpOnly: true, sameSite: 'Lax' }]);
    await context.addInitScript(enabled => {
      localStorage.setItem('lang', 'en');
      localStorage.setItem('yahu.provider-usage.enabled.v1', JSON.stringify({ opencode: enabled }));
      class EmptyEvents { close() {} addEventListener() {} removeEventListener() {} }
      window.WebSocket = EmptyEvents; window.EventSource = EmptyEvents;
    }, enabled);
    const page = await context.newPage();
    let errors = 0, writes = 0;
    page.on('pageerror', () => errors++);
    const windows = Array.from({ length: count }, (_, i) => ['5h额度', '周额度', '月额度'].map((period, j) => ({ window: `Account ${i + 1} ${period}`, used: `${(i + 1) * 10 + j}%`, reset: '5小时', reset_at: 2000000000 }))).flat();
    const section = { provider: 'opencode', title: 'OpenCode Go 用量', description: 'Go Plan 额度', rows: [], windows, errors: [], captured_at: Date.now() / 1000 };
    const payload = { fetched_at: Date.now() / 1000, providers: [{ provider: 'opencode', title: section.title, configured: true, query_ready: true, credential_hint: 'auth.json credential_pool.opencode-go', setup_hint: '' }], sections: [section] };
    await page.route('**/*', async route => {
      const req = route.request(), u = new URL(req.url()), p = u.pathname;
      if (u.origin !== base) return route.abort();
      if (req.method() !== 'GET') { writes++; return route.abort(); }
      if (p === '/' || p.startsWith('/assets/') || p.startsWith('/icon') || p.startsWith('/provider-icons/')) return route.continue();
      const body = p === '/provider-usage' ? payload : p === '/sessions/search' || p.startsWith('/skills/') ? { data: [] } : p === '/workspace/list' ? { entries: [], path: '' } : {};
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    });
    await page.goto(base + '/?opencode-accounts-qa=1#/usage');
    const card = page.locator('[data-provider-id="opencode"]');
    await card.waitFor();
    if (enabled) await card.locator('.provider-usage-account-group').last().waitFor();
    assert(!((await card.textContent()) || '').includes('Go Plan 额度'), 'OpenCode plan description is still visible');
    const geometry = await card.evaluate(el => {
      const s = getComputedStyle(el);
      const body = el.querySelector('.provider-usage-card-content');
      const lastQuota = el.querySelector('.provider-usage-account-group:last-child .provider-usage-window:last-child');
      const clipped = !!body && !!lastQuota && lastQuota.getBoundingClientRect().bottom > body.getBoundingClientRect().bottom + 1;
      return { clipped, height: parseFloat(s.height), doubled: el.classList.contains('is-multi-account'), gridRow: s.gridRowStart, groups: [...el.querySelectorAll('.provider-usage-account-group')].map(group => [...group.querySelectorAll('.provider-usage-window')].map(window => { const r = window.getBoundingClientRect(); return { x: r.x, y: r.y }; })), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1 };
    });
    const doubled = enabled && count > 2;
    if (geometry.doubled !== doubled || (enabled && width > 760 && geometry.height !== (doubled ? 510 : 250))) console.log(JSON.stringify({ stage, geometry }));
    assert(geometry.doubled === doubled && (!enabled || width <= 760 || geometry.height === (doubled ? 510 : 250)), 'card height or threshold mismatch');
    assert(!geometry.overflow, 'horizontal overflow');
    if (enabled && count === 2) assert(!geometry.clipped, 'regular two-account quotas are clipped');
    if (doubled) assert(geometry.gridRow === 'span 2', 'card does not span two grid rows');
    if (enabled) {
      assert(geometry.groups.length === count, 'account group missing');
      assert(await card.locator('.provider-usage-window').count() === count * 3, 'quota window missing');
      for (const windows of geometry.groups) {
        assert(windows.length === 3, 'account does not retain all three quotas');
        if (doubled) assert(windows.every(w => Math.abs(w.x - windows[0].x) < 1) && windows[2].y > windows[1].y && windows[1].y > windows[0].y, 'account quotas are not vertical');
        else assert(windows.every(w => Math.abs(w.y - windows[0].y) < 1) && windows[2].x > windows[1].x, 'regular account layout is not horizontal');
      }
    }
    assert(JSON.stringify(assets(await page.content())) === JSON.stringify(expectedAssets), 'deployed assets mismatch');
    assert(errors === 0 && writes === 0, 'browser error or write request');
    if (out && enabled) await card.screenshot({ path: `${out}/opencode-${width}-${count}.png` });
    console.log(JSON.stringify({ viewport: width, accounts: count, enabled, doubleHeight: doubled, verticalQuotas: doubled, height: geometry.height, errors, writes }));
    await context.close();
  }
} catch (error) {
  console.error(JSON.stringify({ failedStage: stage, errorClass: error.constructor.name, assertion: error.constructor === Error ? error.message : undefined }));
  process.exitCode = 1;
} finally { await browser?.close(); }
