#!/usr/bin/env bun
// Isolated synthetic workload: never connects to Hermes or sends chat messages.
import { execFileSync } from 'node:child_process';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

const root = resolve(import.meta.dir, '..');
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const temp = await mkdtemp(join(tmpdir(), 'yahu-ui-perf-'));
const entry = join(temp, 'entry.tsx');
const fixture = `import React from ${JSON.stringify(join(root, 'node_modules/react/index.js'))};
import { createRoot } from ${JSON.stringify(join(root, 'node_modules/react-dom/client.js'))};
import { flushSync } from ${JSON.stringify(join(root, 'node_modules/react-dom/index.js'))};
import { ChatTranscript } from ${JSON.stringify(join(root, 'frontend/src/ChatTranscript.tsx'))};
import { markdownText } from ${JSON.stringify(join(root, 'frontend/src/markdown.ts'))};
const root = createRoot(document.getElementById('root'));
const line = 'Synthetic result with **formatting** and code. '.repeat(160);
const messages = Array.from({length:30}, (_, i) => [
  {id:i+'u',role:'user',content:'Synthetic task '+i},
  ...Array.from({length:6},(_, j)=>({id:i+'t'+j,role:'tool',toolName:'terminal',content:JSON.stringify({tool_name:'terminal',output:line,exit_code:0})})),
  {id:i+'a',role:'assistant',content:'Completed synthetic task '+i},
]).flat();
window.renderHistory = () => flushSync(() => root.render(<ChatTranscript messages={messages} showReasoning showToolCalls />));
window.renderText = (text, pending=true) => {
  window.expectedStreamHtml = markdownText(text);
  flushSync(() => root.render(<ChatTranscript messages={[{id:'stream',role:'assistant',content:text,pending}]} showReasoning showToolCalls streaming={pending} />));
};
window.renderStream = (length) => window.renderText(('Streaming **formatted** text with Unicode 界.\\n\\n').repeat(400).slice(0,length));
window.clearFixture = () => flushSync(() => root.render(null));
window.ready = true;`;

type Measurement = { renderMs: number; nodes: number; fadeSpans: number; detailBodies: number; retainedHeapBytes: number };
let browser: any;
let server: ReturnType<typeof Bun.serve> | undefined;
try {
  await writeFile(entry, fixture);
  const baselineRef = process.env.YAHU_PERF_BASELINE_REF;
  const baseline = baselineRef
    ? execFileSync('git', ['show', `${baselineRef}:frontend/src/ChatTranscript.tsx`], { cwd: root, encoding: 'utf8' })
    : null;
  const built = await Bun.build({
    entrypoints: [entry], target: 'browser', minify: true,
    define: { 'process.env.NODE_ENV': '"production"' },
    plugins: [{ name: 'fixture-react', setup(build) {
      build.onResolve({ filter: /^react\/jsx-runtime$/ }, () => ({ path: join(root, 'node_modules/react/jsx-runtime.js') }));
      if (baseline) build.onLoad({ filter: /ChatTranscript\.tsx$/ }, () => ({ contents: baseline, loader: 'tsx', resolveDir: join(root, 'frontend/src') }));
    } }],
  });
  if (!built.success) throw new Error('Fixture build failed');
  const js = await built.outputs[0].text();
  const css = await readFile(join(root, 'frontend/src/styles.css'), 'utf8');
  server = Bun.serve({ hostname: '127.0.0.1', port: 0, fetch(req) {
    const path = new URL(req.url).pathname;
    if (path === '/fixture.js') return new Response(js, { headers: { 'Content-Type': 'text/javascript' } });
    if (path === '/style.css') return new Response(css, { headers: { 'Content-Type': 'text/css' } });
    return new Response('<!doctype html><html><head><link rel="stylesheet" href="/style.css"></head><body><section class="chat-scroll" style="height:800px;overflow:auto"><div id="root"></div></section><script type="module" src="/fixture.js"></script></body></html>', { headers: { 'Content-Type': 'text/html' } });
  } });
  browser = await chromium.launch({ headless: true, executablePath: process.env.CHROMIUM_EXECUTABLE || undefined, args: ['--no-sandbox'] });
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: 6 });
  await page.goto(`http://127.0.0.1:${server.port}`);
  await page.waitForFunction('window.ready');
  const results: Record<string, unknown> = { cpuThrottle: 6, fixtureTurns: 30, baselineRef: baselineRef || null };
  for (const kind of ['history', 'stream'] as const) {
    await page.evaluate('window.clearFixture()');
    await cdp.send('HeapProfiler.collectGarbage');
    const startHeap = (await cdp.send('Runtime.getHeapUsage')).usedSize;
    const timing = await page.evaluate(async (kind: string) => {
      const start = performance.now();
      if (kind === 'history') (window as any).renderHistory();
      else for (let length = 11000; length <= 12000; length += 100) (window as any).renderStream(length);
      const renderMs = performance.now() - start;
      await new Promise(requestAnimationFrame);
      return { renderMs, nodes: document.querySelectorAll('*').length, fadeSpans: document.querySelectorAll('.stream-fade-char').length, detailBodies: document.querySelectorAll('.turn-detail-body .msg-row').length };
    }, kind);
    await cdp.send('HeapProfiler.collectGarbage');
    results[kind] = { ...timing, retainedHeapBytes: (await cdp.send('Runtime.getHeapUsage')).usedSize - startHeap };
    if (kind === 'history') {
      await page.locator('.turn-detail-summary').first().click();
      await page.waitForSelector('.turn-detail-body .msg-row');
      await page.locator('.turn-detail-body .tool-summary').first().click();
      await page.waitForSelector('.tool-detail');
      await page.locator('.turn-detail-summary').first().click();
      await page.locator('.turn-detail-summary').first().click();
      results.expandPreserved = await page.locator('.tool-detail').first().isVisible()
        && (await page.locator('.tool-detail').first().textContent()).includes('Synthetic result');
    }
  }
  // Compare the complete rendered Markdown tree, including old text and Unicode.
  results.markdownParity = await page.evaluate(() => {
    for (const text of [
      'A'.repeat(10000) + '👨‍👩‍👧‍👦e\u0301'.repeat(40),
      '| A | B |\n| --- | --- |\n| **bold** | [link](https://example.com) |\n\n```js\nconst a = 1;\n```',
    ]) {
      (window as any).renderText(text);
      const actual = document.querySelector('.stream-fade-text')!.cloneNode(true) as HTMLElement;
      actual.querySelectorAll('.stream-fade-char').forEach((span) => span.replaceWith(document.createTextNode(span.textContent || '')));
      const expected = document.createElement('div');
      expected.innerHTML = (window as any).expectedStreamHtml;
      if (actual.innerHTML !== expected.innerHTML) return false;
    }
    return true;
  });
  results.stableTail = await page.evaluate(() => {
    (window as any).renderText('A'.repeat(1000) + '👨‍👩‍👧‍👦e\u0301');
    const before = document.querySelector('.stream-fade-char:last-child');
    (window as any).renderText('A'.repeat(1000) + '👨‍👩‍👧‍👦e\u0301Z');
    return before === document.querySelector('.stream-fade-char:nth-last-child(2)');
  });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  results.reducedMotion = await page.locator('.stream-fade-char').first().evaluate((span: Element) => getComputedStyle(span).animationName === 'none');
  await page.evaluate("window.renderText('Completed **Markdown**', false)");
  results.completionPreserved = await page.locator('.stream-fade-char').count() === 0
    && await page.locator('.md-content strong').textContent() === 'Markdown';
  console.log(JSON.stringify(results, null, 2));
  if (process.argv.includes('--check')) {
    if ((results.stream as Measurement).fadeSpans > 192) throw new Error('Streaming animation DOM must remain bounded');
    if ((results.history as Measurement).detailBodies !== 0) throw new Error('Unopened details must not mount tool cards');
    for (const flag of ['expandPreserved', 'markdownParity', 'stableTail', 'reducedMotion', 'completionPreserved']) {
      if (!results[flag]) throw new Error(`Rendering regression: ${flag}`);
    }
  }
} finally {
  await browser?.close();
  server?.stop(true);
  await rm(temp, { recursive: true, force: true });
}
