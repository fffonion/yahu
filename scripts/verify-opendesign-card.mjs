// Deployed check for the OpenDesign usage card: presence, setup state, icon, geometry.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { readFile, mkdir } from 'node:fs/promises';
const base = process.env.YAHU_BASE_URL || 'http://127.0.0.1:9642';
const out = process.env.YAHU_QA_OUT;
if (out) await mkdir(out, { recursive: true });
const assert = (ok, msg) => { if (!ok) throw new Error(msg); };
let browser;
try {
  const env = await readFile(process.env.YAHU_ENV_FILE || '/etc/yahu.env', 'utf8');
  const raw = env.match(/^HERMES_WEBUI_AUTH_KEY=(.*)$/m)?.[1]?.trim();
  const key = /^(['"]).*\1$/.test(raw) ? raw.slice(1, -1) : raw;
  const login = await fetch(base + '/login', { method: 'POST', redirect: 'manual', body: new URLSearchParams({ password: key }) });
  const cookie = login.headers.get('set-cookie')?.split(';')[0];
  assert(login.status === 303 && cookie, 'login failed');
  const split = cookie.indexOf('=');
  browser = await chromium.launch({ executablePath: process.env.CHROMIUM_EXECUTABLE || undefined, headless: true, args: ['--no-sandbox'] });
  for (const width of [1440, 390]) {
    const context = await browser.newContext({ viewport: { width, height: 950 }, isMobile: width === 390, hasTouch: width === 390, serviceWorkers: 'block' });
    await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: base, httpOnly: true, sameSite: 'Lax' }]);
    await context.addInitScript(() => localStorage.setItem('lang', 'zh'));
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(base + '/?opendesign-qa=1#/usage');
    const card = page.locator('[data-provider-id="opendesign"]');
    await card.waitFor({ timeout: 30000 });
    await card.scrollIntoViewIfNeeded();
    await page.waitForFunction(() => {
      const img = document.querySelector('[data-provider-id="opendesign"] .provider-usage-brand-icon img');
      return Boolean(img && img.complete && img.naturalWidth > 0);
    }, null, { timeout: 15000 });
    await page.waitForTimeout(300);
    const state = await card.evaluate((el) => {
      const img = el.querySelector('.provider-usage-brand-icon img');
      const css = getComputedStyle(el);
      return {
        title: el.querySelector('h2')?.textContent,
        image: img?.getAttribute('src'),
        imageLoaded: Boolean(img && img.complete && img.naturalWidth > 0),
        height: parseFloat(css.height),
        errors: [...el.querySelectorAll('.provider-usage-errors li')].map((li) => li.textContent),
        body: (el.textContent || '').slice(0, 220),
        overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
      };
    });
    assert(state.title === 'OpenDesign 用量', 'title mismatch');
    assert(state.image === '/provider-icons/opendesign', 'icon path mismatch');
    assert(state.imageLoaded, 'provider icon did not load');
    assert(!state.overflow, 'horizontal overflow');
    if (width > 760) assert(state.height === 250, 'desktop card height changed');
    if (out) await card.screenshot({ path: `${out}/opendesign-${width}.png` });
    console.log(JSON.stringify({ width, height: state.height, imageLoaded: state.imageLoaded, setupError: state.errors[0] || '', pageErrors: errors.length }));
    await context.close();
  }
} catch (error) {
  console.error(JSON.stringify({ errorClass: error.constructor.name, message: error.message }));
  process.exitCode = 1;
} finally { await browser?.close(); }
