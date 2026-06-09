#!/usr/bin/env node
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';

const ROOT = resolve(new URL('..', import.meta.url).pathname);
const OUT_DIR = join(ROOT, 'docs', 'screenshots');
const PORT = Number(process.env.YAHU_SCREENSHOT_PORT || 9765);
const BASE = `http://127.0.0.1:${PORT}`;
const YAHU_BIN = process.env.YAHU_BIN || (existsSync(join(ROOT, 'target', 'release', 'yahu')) ? join(ROOT, 'target', 'release', 'yahu') : `${process.env.HOME}/.local/bin/yahu`);

const now = 1_766_700_000;
const desktop = { width: 1440, height: 900 };
const mobile = { width: 390, height: 844, isMobile: true, hasTouch: true };

function total({ sessions = 4, input = 1_200_000, output = 140_000, cache_read = 5_200_000, cache_write = 80_000, reasoning = 24_000, api_calls = 42, tool_calls = 18, cost_usd = 1.32, unpriced_tokens = 0 }) {
  const total_tokens = input + output + cache_read + cache_write + reasoning;
  return {
    sessions,
    input,
    output,
    cache_read,
    cache_write,
    reasoning,
    api_calls,
    tool_calls,
    estimated_cost_usd: cost_usd,
    actual_cost_usd: 0,
    cost_usd,
    unpriced_tokens,
    total_tokens,
    cache_hit_rate: cache_read / Math.max(1, cache_read + input),
    avg_tokens_per_session: total_tokens / Math.max(1, sessions),
  };
}
function addTotals(a, b) {
  const out = total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 });
  for (const k of Object.keys(out)) out[k] = (a[k] || 0) + (b[k] || 0);
  out.cache_hit_rate = out.cache_read / Math.max(1, out.cache_read + out.input);
  out.avg_tokens_per_session = out.total_tokens / Math.max(1, out.sessions);
  return out;
}
function makeInsights() {
  const dates = ['06/03', '06/04', '06/05', '06/06', '06/07', '06/08', '06/09'];
  const models = [
    ['openai/gpt-5.5', 1.0, 5.4],
    ['minimax/minimax-m3', 0.72, 3.7],
    ['deepseek/deepseek-v4-flash', 0.38, 1.9],
    ['xai/grok-4.3', 0.24, 2.5],
  ].map(([model, scale, cost]) => {
    const daily = dates.map((label, i) => ({
      date: `2026-06-${String(i + 3).padStart(2, '0')}`,
      label,
      totals: total({
        sessions: Math.round(3 + i * Number(scale)),
        input: Math.round((620_000 + i * 98_000) * Number(scale)),
        output: Math.round((82_000 + i * 14_000) * Number(scale)),
        cache_read: Math.round((3_600_000 + i * 410_000) * Number(scale)),
        cache_write: Math.round(24_000 * Number(scale)),
        reasoning: Math.round((12_000 + i * 2_500) * Number(scale)),
        api_calls: Math.round(18 + i * 3 * Number(scale)),
        tool_calls: Math.round(8 + i * Number(scale)),
        cost_usd: Number(cost) * (0.7 + i / 10),
      }),
    }));
    const totals = daily.reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
    return { model, totals, daily };
  });
  const daily = dates.map((label, i) => {
    const totals = models.map((m) => m.daily[i].totals).reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
    return { date: `2026-06-${String(i + 3).padStart(2, '0')}`, label, totals };
  });
  const totals = daily.map((d) => d.totals).reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
  return {
    object: 'usage_insights',
    generated_at: now,
    window_days: 30,
    totals,
    daily,
    models,
    sources: [
      { source: 'telegram', totals: addTotals(models[0].totals, models[1].totals) },
      { source: 'webui', totals: models[2].totals },
      { source: 'cron', totals: models[3].totals },
    ],
    periods: [1, 7, 30].map((days) => ({ days, totals, models })),
  };
}

const insights = makeInsights();
const sessions = [
  { id: 'demo-chat-001', title: 'Demo release QA with tools', preview: 'Terminal, file, and browser tools summarized in one place.', started_at: now - 5600, ended_at: now - 4300, last_active: now - 4200, message_count: 7, model: 'openai/gpt-5.5', provider: 'openai' },
  { id: 'demo-chat-002', title: 'Design review notes', preview: 'Compared mobile spacing and theme tokens.', started_at: now - 14_400, last_active: now - 13_800, message_count: 5, model: 'minimax/minimax-m3', provider: 'minimax' },
  { id: 'demo-chat-003', title: 'Gallery refresh audit', preview: 'Checked image watcher deltas and download actions.', started_at: now - 22_000, last_active: now - 21_500, message_count: 8, model: 'deepseek/deepseek-v4-flash', provider: 'deepseek' },
];
const messages = [
  { id: '1', role: 'user', timestamp: now - 5400, content: 'Can you inspect the demo workspace, run the UI checks, and verify the gallery refresh flow?' },
  { id: '2', role: 'assistant', timestamp: now - 5320, content: 'I will check the project state, inspect the Rust fixture, and verify the UI paths with browser evidence.' },
  { id: '3', role: 'tool', tool_name: 'terminal', timestamp: now - 5280, content: JSON.stringify({ tool_name: 'terminal', command: 'bun test frontend/src/*.test.ts', output: '150 tests passed across 39 files in 0.22s', exit_code: 0 }) },
  { id: '4', role: 'tool', tool_name: 'read_file', timestamp: now - 5220, content: JSON.stringify({ tool_name: 'read_file', path: '/demo/workspace/src/main.rs', content: 'pub fn render_dashboard(config: DemoConfig) -> Result<Page> { /* highlighted in Workspace */ }' }) },
  { id: '5', role: 'tool', tool_name: 'browser_navigate', timestamp: now - 5160, content: JSON.stringify({ tool_name: 'browser_navigate', status: 'success', url: 'http://127.0.0.1:9765/#/insights', title: 'Insights' }) },
  { id: '6', role: 'tool', tool_name: 'image_generate', timestamp: now - 5100, content: JSON.stringify({ tool_name: 'image_generate', image: 'demo-gallery://animals', message: 'Generated varied public-safe animal placeholders for the gallery fixture.' }) },
  { id: '7', role: 'assistant', timestamp: now - 5040, content: '**Done.** The demo build is green, the Rust file preview is highlighted, and the gallery has varied placeholder art with refresh-ready metadata.' },
];
const jobs = [
  { job_id: 'job-demo-weekly', name: 'Weekly usage digest', schedule: '0 9 * * MON', status: 'active', prompt: 'Summarize the last week of demo model usage. Highlight cache efficiency, top models, and cost trend changes. Return three bullets and one follow-up action.', script: 'scripts/collect_usage_demo.py', deliver: 'telegram:#demo' },
  { job_id: 'job-gallery-refresh', name: 'Gallery refresh watcher', schedule: 'every 2h', status: 'active', prompt: 'Check the demo image directory for new public placeholder art and summarize any changed metadata.', script: 'scripts/check_gallery_demo.py', deliver: 'local' },
  { job_id: 'job-skill-audit', name: 'Skill docs audit', schedule: '0 17 * * FRI', status: 'paused', prompt: 'Review demo skill docs for outdated commands and missing verification steps.', script: '', deliver: 'discord:#ops-demo' },
];
const skills = [
  { name: 'demo-gallery-curator', category: 'productivity', description: 'Curate public-safe gallery fixtures and screenshot assets.', enabled: true },
  { name: 'demo-rust-workspace', category: 'software-development', description: 'Inspect Rust workspace structure and highlight source previews.', enabled: true },
  { name: 'demo-browser-qa', category: 'software-development', description: 'Run browser layout checks with mocked UI fixtures.', enabled: true },
];
const skillFiles = {
  '': [
    { name: 'SKILL.md', path: 'SKILL.md', kind: 'file', size: 1540 },
    { name: 'references', path: 'references', kind: 'dir' },
    { name: 'scripts', path: 'scripts', kind: 'dir' },
  ],
  references: [{ name: 'gallery-fixtures.md', path: 'references/gallery-fixtures.md', kind: 'file', size: 720 }],
  scripts: [{ name: 'prepare-demo-gallery.py', path: 'scripts/prepare-demo-gallery.py', kind: 'file', size: 960 }],
};
const skillFileBodies = {
  'SKILL.md': `---\nname: demo-gallery-curator\ndescription: Public-safe screenshot fixture workflow.\n---\n\n# Demo Gallery Curator\n\nUse deterministic placeholder art for public documentation.\n\n## Steps\n\n1. Generate varied animal thumbnails.\n2. Capture gallery and chat with mocked data.\n3. Verify no live session, skill, cron, or workspace content is visible.`,
  'references/gallery-fixtures.md': '# Gallery fixtures\n\nUse simple cat, dog, fox, bird, and rabbit illustrations. Avoid logos and real people.',
  'scripts/prepare-demo-gallery.py': 'def build_gallery_fixture():\n    return ["cat", "dog", "fox", "bird"]\n',
};
const workspaceEntries = {
  '': [
    { name: 'src', path: 'src', kind: 'dir' },
    { name: 'Cargo.toml', path: 'Cargo.toml', kind: 'file', size: 512 },
    { name: 'README.md', path: 'README.md', kind: 'file', size: 1120 },
  ],
  src: [
    { name: 'main.rs', path: 'src/main.rs', kind: 'file', size: 1840 },
    { name: 'lib.rs', path: 'src/lib.rs', kind: 'file', size: 920 },
    { name: 'ui.rs', path: 'src/ui.rs', kind: 'file', size: 1380 },
  ],
};
const workspaceFiles = {
  'src/main.rs': `use anyhow::Result;\nuse demo_ui::{Dashboard, Theme};\n\n#[derive(Debug, Clone)]\npub struct DemoConfig {\n    pub title: String,\n    pub theme: Theme,\n    pub refresh_seconds: u64,\n}\n\npub async fn render_dashboard(config: DemoConfig) -> Result<Dashboard> {\n    let mut dashboard = Dashboard::new(&config.title);\n    dashboard.set_theme(config.theme);\n    dashboard.add_card("Usage", "7.4M tokens", "95% cache hit");\n    dashboard.add_card("Tools", "terminal · read_file · browser", "all verified");\n\n    if config.refresh_seconds > 0 {\n        dashboard.enable_live_refresh(config.refresh_seconds);\n    }\n\n    Ok(dashboard)\n}\n\n#[tokio::main]\nasync fn main() -> Result<()> {\n    let config = DemoConfig {\n        title: "Yahu demo workspace".into(),\n        theme: Theme::VsCodeDarkPlus,\n        refresh_seconds: 30,\n    };\n    render_dashboard(config).await?;\n    Ok(())\n}\n`,
  'src/lib.rs': 'pub mod ui;\npub mod demo;\n',
  'Cargo.toml': '[package]\nname = "demo-yahu-fixture"\nversion = "0.1.0"\nedition = "2021"\n',
  'README.md': '# Demo workspace\n\nSynthetic workspace content for public screenshots.\n',
};

const palettes = [
  ['#0ea5e9', '#fef3c7', '#f97316'],
  ['#8b5cf6', '#fce7f3', '#22c55e'],
  ['#14b8a6', '#ecfeff', '#f59e0b'],
  ['#ef4444', '#fff7ed', '#3b82f6'],
  ['#84cc16', '#f0fdf4', '#a855f7'],
  ['#06b6d4', '#eff6ff', '#fb7185'],
  ['#f59e0b', '#fffbeb', '#10b981'],
  ['#6366f1', '#eef2ff', '#f43f5e'],
  ['#22c55e', '#f7fee7', '#0ea5e9'],
  ['#ec4899', '#fdf2f8', '#14b8a6'],
  ['#eab308', '#fefce8', '#7c3aed'],
  ['#0f766e', '#ccfbf1', '#ea580c'],
];
const animalNames = ['cat-orbit', 'dog-lantern', 'fox-river', 'rabbit-moon', 'bird-garden', 'cat-cloud', 'dog-comet', 'fox-sunrise', 'rabbit-meadow', 'bird-aurora', 'cat-citrus', 'dog-tide'];
function animalSvg(name, index) {
  const [main, bg, accent] = palettes[index % palettes.length];
  const isDog = name.includes('dog');
  const isBird = name.includes('bird');
  const isRabbit = name.includes('rabbit');
  const isFox = name.includes('fox');
  const ears = isDog ? `<path d="M145 142 C110 105 104 75 136 74 C166 82 169 118 145 142Z" fill="${accent}"/><path d="M255 142 C290 105 296 75 264 74 C234 82 231 118 255 142Z" fill="${accent}"/>`
    : isRabbit ? `<path d="M158 126 C136 70 151 28 177 72 C188 95 181 118 158 126Z" fill="${main}"/><path d="M242 126 C264 70 249 28 223 72 C212 95 219 118 242 126Z" fill="${main}"/>`
    : isBird ? `<path d="M128 150 C78 124 76 86 132 102 Z" fill="${accent}"/><path d="M272 150 C322 124 324 86 268 102 Z" fill="${accent}"/>`
    : `<path d="M150 128 L116 70 L180 100 Z" fill="${isFox ? accent : main}"/><path d="M250 128 L284 70 L220 100 Z" fill="${isFox ? accent : main}"/>`;
  const snout = isBird ? `<path d="M200 186 L238 204 L200 222 Z" fill="${accent}"/>` : `<ellipse cx="200" cy="214" rx="42" ry="26" fill="${bg}"/><circle cx="184" cy="206" r="5" fill="#111827"/><circle cx="216" cy="206" r="5" fill="#111827"/><path d="M191 222 Q200 232 209 222" fill="none" stroke="#111827" stroke-width="5" stroke-linecap="round"/>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop stop-color="${bg}"/><stop offset="1" stop-color="${main}" stop-opacity=".42"/></linearGradient></defs><rect width="400" height="300" rx="36" fill="url(#g)"/><circle cx="70" cy="56" r="28" fill="${accent}" opacity=".22"/><circle cx="334" cy="236" r="44" fill="${main}" opacity=".18"/><path d="M62 238 C118 202 150 260 202 224 C246 193 289 225 340 188" fill="none" stroke="${accent}" stroke-width="14" stroke-linecap="round" opacity=".34"/>${ears}<ellipse cx="200" cy="166" rx="95" ry="82" fill="${main}"/><circle cx="166" cy="162" r="10" fill="#111827"/><circle cx="234" cy="162" r="10" fill="#111827"/>${snout}<circle cx="150" cy="198" r="12" fill="${accent}" opacity=".35"/><circle cx="250" cy="198" r="12" fill="${accent}" opacity=".35"/></svg>`;
}
const imageAssets = new Map(animalNames.map((name, index) => [`/demo-assets/${name}.svg`, animalSvg(name, index)]));
const images = animalNames.map((name, index) => ({
  filename: `${name}.png`,
  image_url: `/demo-assets/${name}.svg`,
  png_url: `/demo-assets/${name}.svg`,
  heic_url: index % 3 === 0 ? `/demo-assets/${name}.svg` : null,
  heic_status: index % 3 === 0 ? 'available' : 'missing',
  heic_filename: `${name}.heic`,
  download_filename: `${name}.png`,
  download_url: `/demo-assets/${name}.svg`,
  download_label: 'Download',
  created_at: now - index * 600,
  modified_at: now - index * 600,
  size: 42_000 + index * 1800,
}));

function json(data) {
  return { status: 200, contentType: 'application/json', body: JSON.stringify(data) };
}
function text(body, contentType = 'text/plain; charset=utf-8') {
  return { status: 200, contentType, body };
}
function notFound(pathname) {
  return { status: 404, contentType: 'text/plain', body: `No demo fixture for ${pathname}` };
}
function routeFixture(url, method) {
  const pathname = url.pathname;
  if (imageAssets.has(pathname)) return text(imageAssets.get(pathname), 'image/svg+xml; charset=utf-8');
  if (pathname === '/models-cache') return json({ providers: [
    { slug: 'openai', name: 'OpenAI', models: ['openai/gpt-5.5', 'openai/gpt-5-mini'] },
    { slug: 'minimax', name: 'MiniMax', models: ['minimax/minimax-m3'] },
    { slug: 'deepseek', name: 'DeepSeek', models: ['deepseek/deepseek-v4-flash'] },
  ] });
  if (pathname === '/sessions/search') return json({ data: sessions });
  if (pathname === '/chat/messages/demo-chat-001') return json({ data: messages, total: messages.length, has_older: false, has_newer: false });
  if (pathname === '/insights/usage') return json(insights);
  if (pathname === '/skills/list') return json({ data: skills });
  if (pathname === '/skills/files') {
    const dir = url.searchParams.get('path') || '';
    return json({ path: dir, entries: skillFiles[dir] || [] });
  }
  if (pathname === '/skills/file') return text(skillFileBodies[url.searchParams.get('path') || 'SKILL.md'] || 'Demo skill fixture.');
  if (pathname === '/workspace/list') {
    const dir = url.searchParams.get('path') || '';
    return json({ path: dir, entries: workspaceEntries[dir] || [] });
  }
  if (pathname === '/workspace/file') return text(workspaceFiles[url.searchParams.get('path') || 'src/main.rs'] || 'demo fixture');
  if (pathname === '/image-api/stats') return json({ total_images: images.length, total_bytes: images.reduce((sum, item) => sum + item.size, 0) });
  if (pathname === '/image-api/images') {
    const offset = Number(url.searchParams.get('offset') || 0);
    const limit = Number(url.searchParams.get('limit') || 24);
    return json(images.slice(offset, offset + limit));
  }
  if (pathname === '/image-api/images/refresh') return json({ new_items: [], checked_items: images.slice(0, 6) });
  if (pathname.startsWith('/image-api/images/')) {
    const parts = pathname.split('/');
    const filename = decodeURIComponent(parts[3] || '');
    const item = images.find((img) => img.filename === filename);
    if (parts[4] === 'metadata') return json({ filename, dimensions: { width: 400, height: 300 }, png: { filename, url: item?.png_url || '', size: item?.size || 0, modified_at: item?.modified_at || now }, heic_status: item?.heic_status || 'missing', png_text: [{ keyword: 'Prompt', value: 'Public-safe generated animal placeholder.' }, { keyword: 'Theme', value: 'Yahu README demo fixture.' }] });
    if (item) return json(item);
  }
  if (pathname === '/memory') return json({ memory: 'Demo memory placeholder.', user: 'Demo user placeholder.' });
  if (pathname === '/chat/watch/demo-chat-001' || pathname === '/image-api/events') return text(': demo\n\n', 'text/event-stream');
  if (pathname === '/hermes/api/jobs') return json({ data: jobs });
  if (pathname === '/hermes/api/sessions/demo-chat-001') return json({ data: sessions[0] });
  if (pathname.startsWith('/hermes/api/sessions/')) return json({ data: sessions.find((s) => pathname.endsWith(s.id)) || sessions[0] });
  if (pathname.startsWith('/hermes/api/jobs/')) {
    const id = decodeURIComponent(pathname.split('/')[4] || '');
    if (method === 'PATCH' || method === 'DELETE' || pathname.endsWith('/run')) return json({ ok: true });
    return json({ data: jobs.find((job) => job.job_id === id) || jobs[0] });
  }
  return null;
}

async function installDemoRoutes(page) {
  await page.route('**/*', async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.origin !== BASE) return route.continue();
    const fixture = routeFixture(url, request.method());
    if (fixture) return route.fulfill(fixture);
    return route.continue();
  });
}

async function waitForServer() {
  const deadline = Date.now() + 15_000;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`${BASE}/health`);
      if (res.ok) return;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Timed out waiting for ${BASE}/health`);
}

function startYahu() {
  const child = spawn(YAHU_BIN, ['--insecure', '--host', '127.0.0.1', '--port', String(PORT), '--api-url', 'http://127.0.0.1:8642'], { stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', (chunk) => process.stdout.write(`[yahu] ${chunk}`));
  child.stderr.on('data', (chunk) => process.stderr.write(`[yahu] ${chunk}`));
  child.on('exit', (code) => { if (code !== null && code !== 0) console.error(`yahu exited with ${code}`); });
  return child;
}

async function capturePage(browser, spec) {
  const context = await browser.newContext({ viewport: spec.viewport, deviceScaleFactor: spec.mobile ? 2 : 1, isMobile: !!spec.mobile, hasTouch: !!spec.mobile });
  await context.addInitScript(({ theme }) => {
    localStorage.setItem('theme', theme);
    localStorage.setItem('lang', 'en');
    localStorage.setItem('apiBase', '/hermes');
    localStorage.setItem('model', 'openai/gpt-5.5');
    class DemoEventSource {
      constructor(url) { this.url = url; this.readyState = 1; setTimeout(() => this.onopen && this.onopen({ type: 'open' }), 15); }
      addEventListener() {}
      removeEventListener() {}
      close() { this.readyState = 2; }
    }
    window.EventSource = DemoEventSource;
  }, { theme: spec.theme });
  const page = await context.newPage();
  await installDemoRoutes(page);
  await page.goto(`${BASE}/?demo=${spec.name}${spec.hash}`, { waitUntil: 'networkidle' });
  if (spec.prepare) await spec.prepare(page);
  await page.waitForTimeout(spec.mobile ? 700 : 900);
  await page.screenshot({ path: join(OUT_DIR, spec.file), fullPage: false });
  await context.close();
  console.log(`captured ${spec.file}`);
}

const specs = [
  { name: 'chat', file: 'chat.png', hash: '#/chat/demo-chat-001', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.tool-card'); await page.evaluate(() => document.querySelector('.chat-scroll')?.scrollTo(0, document.querySelector('.chat-scroll')?.scrollHeight || 0)); } },
  { name: 'insights', file: 'insights.png', hash: '#/insights', theme: 'vscode-light-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.usage-chart svg'); } },
  { name: 'skills', file: 'skills.png', hash: '#/skills/demo-gallery-curator', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.workspace-code-highlight'); await page.locator('.section-label', { hasText: 'productivity' }).click().catch(() => {}); } },
  { name: 'cron', file: 'cron.png', hash: '#/cron/job-demo-weekly', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.cron-detail textarea'); } },
  { name: 'workspace', file: 'workspace.png', hash: '#/workspace/file/src%2Fmain.rs', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.workspace-code-highlight .tok-keyword'); } },
  { name: 'gallery', file: 'gallery.png', hash: '#/images', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.image-card img.loaded'); } },
  { name: 'chat-mobile', file: 'chat-mobile.png', hash: '#/chat/demo-chat-001', theme: 'vscode-dark-plus', viewport: mobile, mobile: true, prepare: async (page) => { await page.waitForSelector('.tool-card'); await page.evaluate(() => document.querySelector('.chat-scroll')?.scrollTo(0, document.querySelector('.chat-scroll')?.scrollHeight || 0)); } },
  { name: 'insights-mobile', file: 'insights-mobile.png', hash: '#/insights', theme: 'vscode-dark-plus', viewport: mobile, mobile: true, prepare: async (page) => { await page.waitForSelector('.usage-chart svg'); } },
];

mkdirSync(OUT_DIR, { recursive: true });
for (const file of ['chat.png', 'cron.png', 'workspace.png', 'skills.png', 'images.png', 'insights.png', 'gallery.png', 'chat-mobile.png', 'insights-mobile.png']) {
  const path = join(OUT_DIR, file);
  if (existsSync(path)) rmSync(path);
}
const server = startYahu();
let browser;
try {
  await waitForServer();
  browser = await chromium.launch({ headless: true });
  for (const spec of specs) await capturePage(browser, spec);
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
