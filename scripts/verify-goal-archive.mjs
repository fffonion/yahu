// Read-only QA of deployed assets with synthetic Goal snapshots.
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
import { mkdir, readFile } from 'node:fs/promises';
const base = process.env.YAHU_BASE_URL || 'http://127.0.0.1:9642';
const out = process.env.YAHU_QA_OUT;
if (out) await mkdir(out, { recursive: true });
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const now = Math.floor(Date.now() / 1000);
const done = {
  text: 'Deliver the verified release', status: 'done', created_at: now - 5520, last_turn_at: now - 120,
  turns_used: 8, max_turns: 20, subgoals: ['Keep the original functionality', 'Verify desktop and mobile'],
  todos: [{ id: 'test', content: 'Run the complete test suite', status: 'completed' }, { id: 'release', content: 'Verify the deployed release', status: 'completed' }],
  milestones: [
    { turn: 2, timestamp: now - 3720, verdict: 'continue', reason: 'Implementation ready' },
    { turn: 5, timestamp: now - 1920, verdict: 'continue', reason: 'Regression checks passed' },
    { turn: 8, timestamp: now - 120, verdict: 'done', reason: 'Release verified successfully' },
  ],
  last_reason: 'Release verified successfully',
};
const session = { id: 'goal-archive-fixture', title: 'Example goal showcase', source: 'api', model: 'example', message_count: 2, started_at: now - 5600, last_active: now };
const messages = [{ id: '1', role: 'user', content: 'Verify the example release.', timestamp: now - 5600 }, { id: '2', role: 'assistant', content: 'The example release has been verified.', timestamp: now }];
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
  for (const width of [1280, 390]) for (const theme of ['hermes-dark', 'hermes-light']) {
    stage = `${width}-${theme}`;
    const context = await browser.newContext({ viewport: { width, height: 844 }, isMobile: width === 390, hasTouch: width === 390, serviceWorkers: 'block' });
    await context.addCookies([{ name: cookie.slice(0, split), value: cookie.slice(split + 1), url: base, httpOnly: true, sameSite: 'Lax' }]);
    await context.addInitScript(({ done, session, theme }) => {
      localStorage.setItem('lang', 'en'); localStorage.setItem('theme', theme);
      window.__goalSnapshot = goal => ({ type: 'subagents.snapshot', session_id: session.id, generated_at: Date.now() / 1000, goal, subagents: [] });
      class FixtureSocket {
        constructor(url) {
          if (String(url).includes('/chat/subagents/')) {
            window.__goalSocket = this;
            setTimeout(() => this.onmessage?.({ data: JSON.stringify(window.__goalSnapshot(done)) }), 30);
          }
        }
        close() {} addEventListener() {} removeEventListener() {}
      }
      window.WebSocket = FixtureSocket; window.EventSource = FixtureSocket;
    }, { done, session, theme });
    const page = await context.newPage();
    let errors = 0, writes = 0;
    page.on('pageerror', () => errors++);
    await page.route('**/*', async route => {
      const request = route.request(), u = new URL(request.url()), p = u.pathname;
      if (u.origin !== base) return route.abort();
      if (request.method() !== 'GET') { writes++; return route.abort(); }
      if (p === '/' || p.startsWith('/assets/') || p.startsWith('/icon') || p === '/manifest.json') return route.continue();
      let body = {};
      if (p === '/sessions/search') body = { data: [session] };
      if (p.startsWith('/chat/messages/')) body = { data: messages, total: 2, has_older: false, has_newer: false };
      if (p.startsWith('/chat/user-nav/')) body = { data: [], total: 2 };
      if (p.endsWith('/canonical')) body = { id: session.id, canonical_id: session.id };
      if (p.startsWith('/hermes/api/sessions/')) body = { data: session };
      if (p.endsWith('/status')) body = { running: false };
      if (p.endsWith('/reasoning')) body = { reasoning_effort: 'medium' };
      if (p === '/models-cache') body = { providers: [{ slug: 'example', name: 'Example', models: ['example'] }] };
      if (p === '/workspace/list') body = { path: '', entries: [] };
      if (p.startsWith('/skills/')) body = { data: [] };
      if (p.startsWith('/chat/subagents/') && p.endsWith('/snapshot')) body = { type: 'subagents.snapshot', session_id: session.id, goal: done, subagents: [] };
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    });
    page.setDefaultTimeout(10_000);
    await page.clock.install();
    await page.goto(base + '/?goal-archive-qa=1#/chat/' + session.id);
    stage = `${width}-${theme}:initial-goal`;
    const panel = page.locator('.subagent-goal-panel');
    await panel.waitFor({ state: 'visible' });
    await page.waitForFunction(() => document.querySelector('.subagent-goal-panel')?.dataset.status === 'done');
    const emit = goal => page.evaluate(goal => window.__goalSocket.onmessage({ data: JSON.stringify(window.__goalSnapshot(goal)) }), goal);
    stage = `${width}-${theme}:active-transition`;
    await emit({ ...done, status: 'active' });
    await page.waitForFunction(() => document.querySelector('.subagent-goal-panel')?.dataset.status === 'active');
    stage = `${width}-${theme}:expand`;
    await panel.locator('summary').click();
    const activeColors = await panel.evaluate(e => ({ title: getComputedStyle(e.querySelector('.subagent-goal-preview')).color, subgoal: getComputedStyle(e.querySelector('.subagent-goal-subgoals li')).color, completedTodo: getComputedStyle(e.querySelector('.subagent-progress-todos li.completed')).color }));
    assert(activeColors.title !== activeColors.completedTodo && activeColors.subgoal !== activeColors.completedTodo, 'unfinished Goal was colored as completed');
    stage = `${width}-${theme}:done-transition`;
    await emit(done);
    await page.waitForFunction(() => document.querySelector('.subagent-goal-panel')?.dataset.status === 'done');
    assert(await panel.evaluate(e => e.open), 'completion lost expanded state');
    const metadata = await panel.locator('.subagent-goal-meta').textContent();
    assert(metadata.includes('Done') && metadata.includes('1 hr 30 min') && metadata.includes('8/20 turns'), 'completed metadata missing');
    const detail = await panel.locator('.subagent-goal-body').textContent();
    for (const expected of [...done.subgoals, ...done.todos.map(t => t.content), ...done.milestones.map(m => m.reason)]) assert(detail.includes(expected), 'recorded step missing');
    const geometry = await panel.evaluate(e => {
      const r = e.getBoundingClientRect(), style = getComputedStyle(e), preview = e.querySelector('.subagent-goal-preview');
      const sample = document.createElement('span'); sample.style.color = 'var(--green)'; document.body.append(sample);
      const green = getComputedStyle(sample).color; sample.remove();
      const todoGreen = getComputedStyle(e.querySelector('.subagent-progress-todos li.completed')).color;
      const completedParts = e.querySelectorAll('.subagent-goal-preview,.subagent-goal-meta,.subagent-goal-footer,.subagent-goal-subgoals li,.subagent-goal-milestones li p,.subagent-progress-todos li.completed .subagent-todo-box');
      const sameCompletionGreen = todoGreen === green && [...completedParts].every(node => getComputedStyle(node).color === todoGreen);
      return { top: r.top, bottom: r.bottom, width: r.width, sameCompletionGreen, green: getComputedStyle(preview).color === green, accent: style.getPropertyValue('--subagent-goal-accent').trim(), overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth + 1 };
    });
    assert(geometry.sameCompletionGreen, 'completed Goal content does not match Todo green');
    assert(geometry.green && !geometry.overflow && geometry.top >= 0 && geometry.bottom <= 844, 'completed geometry or color mismatch');
    stage = `${width}-${theme}:expanded-capture`;
    if (out) await panel.screenshot({ path: `${out}/goal-${width}-${theme}-expanded.png` });
    await page.clock.setSystemTime(new Date(Date.now() + 3 * 60 * 60 * 1000));
    await emit(done);
    assert(await panel.locator('.subagent-goal-meta').textContent() === metadata, 'completed timer continued advancing');
    stage = `${width}-${theme}:collapse`;
    const chatBounds = await page.locator('.chat-scroll').boundingBox();
    const clickPoint = { x: chatBounds.x + chatBounds.width - 12, y: chatBounds.y + chatBounds.height - 12 };
    const target = await page.evaluate(({ x, y }) => { const e = document.elementFromPoint(x, y); return { tag: e?.tagName, className: e?.className, inChat: !!e?.closest('.chat-scroll') }; }, clickPoint);
    assert(target.inChat, 'collapse target is outside chat history');
    await page.mouse.click(clickPoint.x, clickPoint.y);
    await page.waitForFunction(() => !document.querySelector('.subagent-goal-panel')?.open);
    assert(!await panel.evaluate(e => e.open), 'chat click did not collapse goal');
    assert(await panel.isVisible(), 'collapsed completed goal disappeared');
    if (out) await panel.screenshot({ path: `${out}/goal-${stage}-collapsed.png` });
    stage = `${width}-${theme}:reload`;
    await page.reload();
    await page.waitForFunction(() => document.querySelector('.subagent-goal-panel')?.dataset.status === 'done');
    assert(await panel.locator('.subagent-goal-meta').textContent() === metadata, 'reload lost the archived duration');
    assert(JSON.stringify(assets(await page.content())) === JSON.stringify(expectedAssets), 'deployed assets mismatch');
    assert(errors === 0 && writes === 0, 'browser error or write request');
    console.log(JSON.stringify({ viewport: width, theme, completedVisible: true, green: geometry.green, sameCompletionGreen: geometry.sameCompletionGreen, frozenDuration: true, stepsPreserved: true, survivesReload: true, errors, writes }));
    await context.close();
  }
} catch (error) {
  console.error(JSON.stringify({ failedStage: stage, errorClass: error.constructor.name, assertion: error.constructor === Error ? error.message : undefined }));
  process.exitCode = 1;
} finally { await browser?.close(); }
