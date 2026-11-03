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
  for (const width of [1440, 390]) for (const [count, enabled] of [[1, true], [2, true], [3, true], [4, true], [2, false], [4, false]]) {
    stage = `${width}-${count}-${enabled}`;
    const context = await browser.newContext({ viewport: { width, height: 900 }, isMobile: width === 390, hasTouch: width === 390, serviceWorkers: 'block' });
    await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: base, httpOnly: true, sameSite: 'Lax' }]);
    await context.addInitScript(enabled => {
      localStorage.setItem('lang', 'en');
      localStorage.setItem('yahu.provider-usage.enabled.v1', JSON.stringify({ opencode: enabled, commandcode: enabled }));
      class EmptyEvents { close() {} addEventListener() {} removeEventListener() {} }
      window.WebSocket = EmptyEvents; window.EventSource = EmptyEvents;
    }, enabled);
    const page = await context.newPage();
    let errors = 0, writes = 0;
    page.on('pageerror', () => errors++);
    const windows = Array.from({ length: count }, (_, i) => ['5h额度', '周额度', '月额度'].map((period, j) => ({ window: `Account ${i + 1} ${period}`, used: `${j === 2 ? (i + 1) * 10 : i % 3 === 0 ? [65, 20][j] : i % 3 === 1 ? [12, 72][j] : 40}%`, reset: '5小时', reset_at: 2000000000 }))).flat();
    const section = { provider: 'opencode', title: 'OpenCode Go 用量', description: 'Go Plan 额度', rows: [], windows, errors: [], captured_at: Date.now() / 1000 };
    const reference = { ...section, provider: 'commandcode', title: 'CommandCode 用量', description: '' };
    const payload = { fetched_at: Date.now() / 1000, providers: [section, reference].map(s => ({ provider: s.provider, title: s.title, configured: true, query_ready: true, credential_hint: '', setup_hint: '' })), sections: [section, reference] };
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
    if (enabled) {
      await card.locator('.provider-usage-account-group').last().waitFor();
      await page.locator('[data-provider-id="commandcode"] .provider-usage-account-group').last().waitFor();
    }
    assert(!((await card.textContent()) || '').includes('Go Plan 额度'), 'OpenCode plan description is still visible');
    const geometry = await card.evaluate(el => {
      const s = getComputedStyle(el);
      return { height: parseFloat(s.height), doubled: el.classList.contains('is-multi-account'), gridRow: s.gridRowStart, groups: [...el.querySelectorAll('.provider-usage-account-group')].map(group => [...group.querySelectorAll('.provider-usage-window')].map(window => { const r = window.getBoundingClientRect(); return { x: r.x, y: r.y, label: window.querySelector('strong')?.textContent, used: window.querySelector('.provider-usage-window-head span')?.textContent, reset: window.querySelector('.provider-usage-window-reset')?.textContent }; })), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1 };
    });
    const doubled = enabled && count >= 3;
    if (geometry.doubled !== doubled || (enabled && width > 760 && geometry.height !== (doubled ? 510 : 250))) console.log(JSON.stringify({ stage, geometry }));
    assert(geometry.doubled === doubled && (!enabled || width <= 760 || geometry.height === (doubled ? 510 : 250)), 'card height or threshold mismatch');
    assert(!geometry.overflow, 'horizontal overflow');
    if (enabled) {
      const styles = await page.evaluate(() => [...document.querySelectorAll('[data-provider-id]')].map(card => {
        const group = card.querySelector('.provider-usage-account-group');
        const stack = group?.querySelector('.provider-usage-windows');
        const quota = group?.querySelector('.provider-usage-window');
        if (!group || !stack || !quota) return null;
        const g = getComputedStyle(group), s = getComputedStyle(stack), q = getComputedStyle(quota);
        return { provider: card.getAttribute('data-provider-id'), layout: [g.padding, s.display, s.flexDirection, s.gap, q.padding] };
      }).filter(Boolean));
      const actual = styles.find(s => s.provider === 'opencode');
      const reference = styles.find(s => s.provider === 'commandcode');
      assert(actual && reference && JSON.stringify(actual.layout) === JSON.stringify(reference.layout), 'account strip styles differ from CommandCode');
      const sharedRadius = await page.locator('[data-provider-id="commandcode"] .provider-usage-window').first().evaluate(el => getComputedStyle(el).borderRadius);
      assert(await card.locator('.provider-usage-account-group').evaluateAll((groups, radius) => groups.every(group => getComputedStyle(group).borderRadius === radius), sharedRadius), 'OpenCode account card radius differs from shared quota cards');
      const cardStructure = await card.locator('.provider-usage-account-group').evaluateAll(groups => groups.every(group => {
        const outer = getComputedStyle(group);
        return parseFloat(outer.borderTopWidth) > 0 && parseFloat(outer.borderRadius) > 0 && [...group.querySelectorAll('.provider-usage-window')].every(window => {
          const inner = getComputedStyle(window);
          return parseFloat(inner.borderTopWidth) === 0 && parseFloat(inner.borderRadius) === 0 && inner.backgroundColor === 'rgba(0, 0, 0, 0)';
        });
      }));
      assert(cardStructure, 'each account must have one outer card without nested quota boxes');
      const quota = card.locator('.provider-usage-account-group:last-child .provider-usage-window:last-child');
      await quota.scrollIntoViewIfNeeded();
      assert(await quota.evaluate(el => { const r = el.getBoundingClientRect(), b = el.closest('.provider-usage-card-content').getBoundingClientRect(); return r.top >= b.top - 1 && r.bottom <= b.bottom + 1; }), 'last quota is inaccessible');
    }
    if (doubled) assert(geometry.gridRow === 'span 2', 'card does not span two grid rows');
    if (enabled) {
      assert(geometry.groups.length === count, 'account group missing');
      assert(await card.locator('.provider-usage-window').count() === count * 2, 'visible quota count mismatch');
      for (const [index, windows] of geometry.groups.entries()) {
        assert(windows.length === 2, 'account must show one short quota and monthly');
        assert(windows.every(w => Math.abs(w.x - windows[0].x) < 1) && windows[1].y > windows[0].y, 'account quotas are not vertical');
        assert(windows[0].label === (index % 3 === 0 ? '5h额度' : '周额度'), 'larger short quota was not selected');
        assert(windows[0].used === (index % 3 === 0 ? '65%' : index % 3 === 1 ? '72%' : '40%'), 'selected quota value changed');
        assert(windows[1].label === '月额度' && windows[1].used === `${(index + 1) * 10}%`, 'monthly quota missing or changed');
        assert(windows.every(w => w.reset), 'quota reset missing');
      }
      if (count >= 2) {
        assert(geometry.groups[1][0].x > geometry.groups[0][0].x && Math.abs(geometry.groups[1][0].y - geometry.groups[0][0].y) < 1, 'account strips are not side by side');
      }
    }
    assert(JSON.stringify(assets(await page.content())) === JSON.stringify(expectedAssets), 'deployed assets mismatch');
    assert(errors === 0 && writes === 0, 'browser error or write request');
    if (enabled) await card.locator('.provider-usage-card-content').evaluate(el => { el.scrollTop = 0; });
    if (out && enabled) await card.screenshot({ path: `${out}/opencode-${width}-${count}.png` });
    console.log(JSON.stringify({ viewport: width, accounts: count, enabled, doubleHeight: doubled, verticalQuotas: enabled, height: geometry.height, errors, writes }));
    await context.close();
  }
} catch (error) {
  console.error(JSON.stringify({ failedStage: stage, errorClass: error.constructor.name, assertion: error.constructor === Error ? error.message : undefined }));
  process.exitCode = 1;
} finally { await browser?.close(); }
