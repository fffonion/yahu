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
  const start = new Date('2026-05-11T00:00:00Z');
  const dayLabels = Array.from({ length: 30 }, (_, i) => {
    const d = new Date(start.getTime() + i * 86_400_000);
    return {
      date: d.toISOString().slice(0, 10),
      label: `${String(d.getUTCMonth() + 1).padStart(2, '0')}/${String(d.getUTCDate()).padStart(2, '0')}`,
      weekday: d.getUTCDay(),
    };
  });
  const profile = [0.48, 0.58, 0.92, 0.76, 1.24, 0.42, 0.36, 0.64, 0.88, 1.38, 0.97, 0.72, 0.54, 0.46, 0.81, 1.12, 0.69, 1.58, 1.04, 0.61, 0.49, 0.73, 0.95, 1.71, 1.18, 0.86, 0.52, 0.44, 1.09, 0.83];
  const modelSpecs = [
    { model: 'openai/gpt-5.5', scale: 1.0, phase: 0, cost: 5.2, toolBias: 1.15 },
    { model: 'minimax/minimax-m3', scale: 0.74, phase: 4, cost: 3.4, toolBias: 0.78 },
    { model: 'deepseek/deepseek-v4-flash', scale: 0.41, phase: 9, cost: 1.8, toolBias: 0.92 },
    { model: 'xai/grok-4.3', scale: 0.28, phase: 15, cost: 2.2, toolBias: 1.34 },
  ];
  const models = modelSpecs.map((spec, modelIndex) => {
    const daily = dayLabels.map((day, i) => {
      const weekdayFactor = day.weekday === 0 || day.weekday === 6 ? 0.52 : 1.0;
      const wave = 1 + 0.18 * Math.sin((i + spec.phase) / 2.7) + 0.11 * Math.cos((i + modelIndex * 3) / 4.1);
      const releasePulse = i === 9 || i === 17 || i === 23 ? 1.42 + modelIndex * 0.08 : 1;
      const quietDip = i === 6 || i === 20 || i === 27 ? 0.58 : 1;
      const activity = Math.max(0.22, profile[i] * weekdayFactor * wave * releasePulse * quietDip * spec.scale);
      const input = Math.round((410_000 + (i % 5) * 42_000 + modelIndex * 31_000) * activity);
      const output = Math.round(input * (0.105 + (i % 4) * 0.008 + modelIndex * 0.006));
      const cache_read = Math.round(input * (16.4 + ((i + modelIndex) % 6) * 0.7));
      const cache_write = Math.round(input * (0.028 + ((i + 2) % 5) * 0.006));
      const reasoning = Math.round(output * (0.13 + ((i + modelIndex) % 4) * 0.025));
      const api_calls = Math.max(1, Math.round(activity * (7 + (i % 6))));
      const tool_calls = Math.max(0, Math.round(activity * (3 + (i % 3)) * spec.toolBias));
      const cost_usd = Number((activity * spec.cost * (0.82 + (i % 7) * 0.035)).toFixed(2));
      return { date: day.date, label: day.label, totals: total({ sessions: Math.max(1, Math.round(api_calls / 3)), input, output, cache_read, cache_write, reasoning, api_calls, tool_calls, cost_usd }) };
    });
    const totals = daily.reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
    return { model: spec.model, totals, daily };
  });
  const daily = dayLabels.map((day, i) => {
    const totals = models.map((m) => m.daily[i].totals).reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
    return { date: day.date, label: day.label, totals };
  });
  const periodTotals = (days) => daily.slice(-days).map((d) => d.totals).reduce(addTotals, total({ sessions: 0, input: 0, output: 0, cache_read: 0, cache_write: 0, reasoning: 0, api_calls: 0, tool_calls: 0, cost_usd: 0 }));
  const totals = periodTotals(30);
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
    periods: [1, 7, 30].map((days) => ({ days, totals: periodTotals(days), models })),
  };
}

const insights = makeInsights();
const sessions = [
  { id: 'demo-chat-001', title: 'Demo release QA with tools', preview: 'Terminal, file, and browser tools summarized in one place.', started_at: now - 7_200, ended_at: now - 4_300, last_active: now - 4_200, message_count: 28, model: 'openai/gpt-5.5', provider: 'openai' },
  { id: 'demo-chat-002', title: 'Design review notes', preview: 'Compared mobile spacing and theme tokens.', started_at: now - 14_400, last_active: now - 13_800, message_count: 5, model: 'minimax/minimax-m3', provider: 'minimax' },
  { id: 'demo-chat-003', title: 'Gallery refresh audit', preview: 'Checked image watcher deltas and download actions.', started_at: now - 22_000, last_active: now - 21_500, message_count: 8, model: 'deepseek/deepseek-v4-flash', provider: 'deepseek' },
];
const turnTopics = [
  ['Scope the README capture', 'Map the pages that need public-safe screenshots and confirm the dummy fixture set.'],
  ['Inspect the demo workspace', 'Open the synthetic Rust file and verify the preview uses highlighting.'],
  ['Run frontend checks', 'Execute the focused UI checks before capturing the public assets.'],
  ['Review chat tool cards', 'Confirm terminal, file, browser, and image tool cards each use distinct icons.'],
  ['Check minimap density', 'Make the user-turn navigator visible with enough compact bars for a long chat.'],
  ['Validate markdown rendering', 'Show a short markdown summary with bold text, a table, and code formatting.'],
  ['Open Insights fixture', 'Verify the 30 day chart has spikes, dips, and multiple model rows.'],
  ['Review Cron fixture', 'Check the latest output preview renders safely with synthetic content only.'],
  ['Audit gallery placeholders', 'Confirm landscape thumbnails vary by palette and scene shape.'],
  ['Check mobile layout', 'Verify the narrow viewport keeps chat content inside the screen width.'],
  ['Refresh screenshot assets', 'Capture the README chat image from a local insecure demo instance.'],
  ['Summarize result', 'Report the capture path and proof that no live data was shown.'],
];
const messages = [];
turnTopics.forEach(([title, prompt], index) => {
  const base = now - 7_200 + index * 210;
  const userId = String(index * 2 + 1);
  const assistantId = String(index * 2 + 2);
  messages.push({ id: userId, role: 'user', timestamp: base, content: `${title}: ${prompt}` });
  if (index === 1) messages.push({ id: `${assistantId}a`, role: 'tool', tool_name: 'read_file', timestamp: base + 22, content: JSON.stringify({ tool_name: 'read_file', path: '/demo/workspace/src/main.rs', content: 'pub async fn render_dashboard(config: DemoConfig) -> Result<Dashboard> { /* highlighted preview */ }' }) });
  if (index === 2) messages.push({ id: `${assistantId}a`, role: 'tool', tool_name: 'terminal', timestamp: base + 22, content: JSON.stringify({ tool_name: 'terminal', command: 'bun test frontend/src/*.test.ts', output: '150 tests passed across 39 files in 0.22s', exit_code: 0 }) });
  if (index === 3) messages.push({ id: `${assistantId}a`, role: 'tool', tool_name: 'browser_navigate', timestamp: base + 22, content: JSON.stringify({ tool_name: 'browser_navigate', status: 'success', url: 'http://127.0.0.1:9765/#/chat/demo-chat-001', title: 'Chat demo' }) });
  if (index === 8) messages.push({ id: `${assistantId}a`, role: 'tool', tool_name: 'image_generate', timestamp: base + 22, content: JSON.stringify({ tool_name: 'image_generate', image: 'demo-gallery://landscapes', message: 'Generated varied public-safe landscape placeholders for the gallery fixture.' }) });
  messages.push({ id: assistantId, role: 'assistant', timestamp: base + 55, content: index === 11 ? '**Done.** The chat screenshot uses dummy data, distinct tool cards, and a dense user-turn minimap.' : `Acknowledged. ${prompt}` });
});
const userNavItems = messages.filter((message) => message.role === 'user').map((message, index, users) => {
  const nextUser = users[index + 1];
  const preview = messages.filter((item) => item.role === 'assistant' && Number(item.id.replace(/\D/g, '') || 0) > Number(String(message.id).replace(/\D/g, '') || 0) && (!nextUser || Number(item.id.replace(/\D/g, '') || 0) < Number(String(nextUser.id).replace(/\D/g, '') || 0))).at(-1);
  return { id: message.id, role: 'user', content: message.content, assistant_preview: preview?.content || '', timestamp: message.timestamp, position: Number(message.id), index: index + 1, total: messages.length };
});
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
  'SKILL.md': `---\nname: demo-gallery-curator\ndescription: Public-safe screenshot fixture workflow.\n---\n\n# Demo Gallery Curator\n\nUse deterministic placeholder art for public documentation.\n\n## Steps\n\n1. Generate varied landscape thumbnails.\n2. Capture gallery and chat with mocked data.\n3. Verify no live session, skill, cron, or workspace content is visible.`,
  'references/gallery-fixtures.md': '# Gallery fixtures\n\nUse simple mountain, coast, forest, desert, lake, and night skyline landscapes. Avoid logos, text, real people, or identifiable places.',
  'scripts/prepare-demo-gallery.py': 'def build_gallery_fixture():\n    return ["alpine_lake", "coastal_dawn", "cedar_valley", "desert_rain"]\n',
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
  ['#0f172a', '#7dd3fc', '#fef3c7', '#f97316'],
  ['#1e1b4b', '#c4b5fd', '#f5d0fe', '#38bdf8'],
  ['#064e3b', '#86efac', '#ecfccb', '#fbbf24'],
  ['#7c2d12', '#fed7aa', '#fef9c3', '#fb7185'],
  ['#1e3a8a', '#93c5fd', '#dbeafe', '#22c55e'],
  ['#312e81', '#818cf8', '#fef3c7', '#f472b6'],
  ['#164e63', '#67e8f9', '#cffafe', '#a3e635'],
  ['#581c87', '#d8b4fe', '#fae8ff', '#f59e0b'],
  ['#365314', '#bef264', '#f7fee7', '#14b8a6'],
  ['#831843', '#f9a8d4', '#fdf2f8', '#38bdf8'],
  ['#78350f', '#fde68a', '#fff7ed', '#10b981'],
  ['#0c4a6e', '#bae6fd', '#f0f9ff', '#f97316'],
];
const landscapeNames = ['alpine-lake', 'coastal-dawn', 'cedar-valley', 'desert-rain', 'fjord-night', 'lavender-hills', 'glacier-bay', 'red-rock-canyon', 'rice-terraces', 'pink-salt-lagoon', 'golden-steppe', 'island-storm'];
function landscapeSvg(name, index) {
  const [dark, sky, haze, accent] = palettes[index % palettes.length];
  const sunX = 62 + (index % 4) * 78;
  const sunY = 54 + (index % 3) * 18;
  const water = name.includes('lake') || name.includes('coastal') || name.includes('fjord') || name.includes('bay') || name.includes('lagoon') || name.includes('island');
  const desert = name.includes('desert') || name.includes('canyon') || name.includes('steppe');
  const trees = name.includes('cedar') || name.includes('terraces') || name.includes('valley');
  const stars = name.includes('night') ? Array.from({ length: 18 }, (_, i) => `<circle cx="${32 + (i * 41) % 336}" cy="${28 + (i * 29) % 96}" r="${1 + (i % 2)}" fill="#fff" opacity=".72"/>`).join('') : '';
  const ridgeA = desert ? `M0 184 C52 122 98 166 142 116 C188 72 230 160 284 102 C330 58 372 126 400 90 V300 H0Z` : `M0 174 C44 120 78 146 118 88 C160 28 214 164 260 94 C304 34 352 112 400 72 V300 H0Z`;
  const ridgeB = `M0 206 C54 166 96 192 142 142 C188 92 230 198 286 150 C336 108 364 154 400 124 V300 H0Z`;
  const foreground = water
    ? `<path d="M0 214 C80 204 142 226 206 214 C282 200 330 222 400 210 V300 H0Z" fill="${haze}" opacity=".82"/><path d="M0 236 C74 224 154 248 230 232 C300 218 346 242 400 230 V300 H0Z" fill="${sky}" opacity=".56"/><path d="M20 252 C86 244 152 264 214 250" fill="none" stroke="#fff" stroke-width="4" opacity=".45"/><path d="M248 260 C298 248 340 262 382 254" fill="none" stroke="#fff" stroke-width="3" opacity=".38"/>`
    : `<path d="M0 224 C60 198 100 226 158 206 C232 180 286 214 400 188 V300 H0Z" fill="${haze}" opacity=".9"/><path d="M0 252 C80 226 146 258 220 234 C300 208 354 238 400 218 V300 H0Z" fill="${accent}" opacity=".34"/>`;
  const treeLayer = trees ? Array.from({ length: 14 }, (_, i) => `<path d="M${18 + i * 28} ${222 - (i % 4) * 9} l10 -34 l10 34 h-20Z" fill="${dark}" opacity=".82"/>`).join('') : '';
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"><defs><linearGradient id="sky" x1="0" x2="0" y1="0" y2="1"><stop stop-color="${sky}"/><stop offset=".58" stop-color="${haze}"/><stop offset="1" stop-color="${dark}" stop-opacity=".42"/></linearGradient><linearGradient id="glow" x1="0" x2="1"><stop stop-color="${accent}" stop-opacity=".85"/><stop offset="1" stop-color="#fff" stop-opacity=".2"/></linearGradient></defs><rect width="400" height="300" rx="34" fill="url(#sky)"/><circle cx="${sunX}" cy="${sunY}" r="32" fill="${accent}" opacity=".72"/><circle cx="${sunX}" cy="${sunY}" r="54" fill="${accent}" opacity=".18"/>${stars}<path d="${ridgeA}" fill="${dark}" opacity=".88"/><path d="${ridgeB}" fill="url(#glow)" opacity=".56"/>${foreground}${treeLayer}<path d="M0 284 C62 272 112 294 176 280 C246 264 314 288 400 272 V300 H0Z" fill="${dark}" opacity=".72"/></svg>`;
}
const landscapeArtwork = landscapeNames.map((name, index) => {
  const svg = landscapeSvg(name, index);
  return { name, svg, url: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}` };
});
const imageAssets = new Map(landscapeArtwork.map(({ name, svg }) => [`/demo-assets/${name}.svg`, svg]));
const images = landscapeArtwork.map(({ name, url }, index) => ({
  filename: `${name}.png`,
  image_url: url,
  png_url: url,
  heic_url: index % 3 === 0 ? url : null,
  heic_status: index % 3 === 0 ? 'available' : 'missing',
  heic_filename: `${name}.heic`,
  download_filename: `${name}.png`,
  download_url: url,
  download_label: 'Download',
  created_at: now - index * 600,
  modified_at: now - index * 600,
  size: 68_000 + index * 2400,
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
  if (pathname === '/chat/user-nav/demo-chat-001') return json({ data: userNavItems, total: messages.length });
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
    if (parts[4] === 'metadata') return json({ filename, dimensions: { width: 400, height: 300 }, png: { filename, url: item?.png_url || '', size: item?.size || 0, modified_at: item?.modified_at || now }, heic_status: item?.heic_status || 'missing', png_text: [{ keyword: 'Prompt', value: 'Public-safe generated landscape placeholder.' }, { keyword: 'Theme', value: 'Yahu README demo fixture.' }] });
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
  const context = await browser.newContext({ viewport: spec.viewport, deviceScaleFactor: spec.mobile ? 2 : 1, isMobile: !!spec.mobile, hasTouch: !!spec.mobile, serviceWorkers: 'block' });
  await context.addInitScript(({ theme }) => {
    if ('serviceWorker' in navigator) navigator.serviceWorker.getRegistrations?.().then((items) => items.forEach((item) => item.unregister())).catch(() => {});
    localStorage.setItem('theme', theme);
    localStorage.setItem('lang', 'en');
    localStorage.setItem('apiBase', '/hermes');
    localStorage.setItem('model', 'openai/gpt-5.5');
    localStorage.setItem('desktopCompactMessages', '1');
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

const waitForChatDemo = async (page) => {
  await page.waitForSelector('.turn-detail-group');
  await page.locator('details.turn-detail-group:not([open]) > summary').evaluateAll((summaries) => summaries.forEach((summary) => summary.click()));
  await page.waitForSelector('.tool-card', { state: 'visible' });
  await page.waitForFunction(() => document.querySelectorAll('.user-minimap-hit').length >= 10);
  await page.evaluate(() => document.querySelector('.chat-scroll')?.scrollTo(0, document.querySelector('.chat-scroll')?.scrollHeight || 0));
};
const specs = [
  { name: 'chat', file: 'chat.png', hash: '#/chat/demo-chat-001', theme: 'vscode-light-plus', viewport: desktop, prepare: waitForChatDemo },
  { name: 'insights', file: 'insights.png', hash: '#/insights', theme: 'vscode-light-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.usage-chart svg'); await page.getByRole('button', { name: '30d' }).click(); await page.waitForTimeout(250); } },
  { name: 'skills', file: 'skills.png', hash: '#/skills/demo-gallery-curator', theme: 'vscode-light-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.workspace-markdown-preview'); await page.locator('.section-label', { hasText: 'productivity' }).click().catch(() => {}); } },
  { name: 'cron', file: 'cron.png', hash: '#/cron/job-demo-weekly', theme: 'vscode-light-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.cron-detail textarea'); } },
  { name: 'workspace', file: 'workspace.png', hash: '#/workspace/file/src%2Fmain.rs', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.workspace-code-highlight .tok-keyword'); } },
  { name: 'gallery', file: 'gallery.png', hash: '#/images', theme: 'vscode-dark-plus', viewport: desktop, prepare: async (page) => { await page.waitForSelector('.image-card img.loaded'); } },
  { name: 'chat-mobile', file: 'chat-mobile.png', hash: '#/chat/demo-chat-001', theme: 'vscode-dark-plus', viewport: mobile, mobile: true, prepare: waitForChatDemo },
  { name: 'insights-mobile', file: 'insights-mobile.png', hash: '#/insights', theme: 'vscode-dark-plus', viewport: mobile, mobile: true, prepare: async (page) => { await page.waitForSelector('.usage-chart svg'); await page.getByRole('button', { name: '30d' }).click(); await page.waitForTimeout(250); } },
];
const only = new Set((process.env.YAHU_SCREENSHOT_ONLY || '').split(',').map((item) => item.trim()).filter(Boolean));
const selectedSpecs = only.size ? specs.filter((spec) => only.has(spec.name) || only.has(spec.file)) : specs;

mkdirSync(OUT_DIR, { recursive: true });
for (const file of ['chat.png', 'cron.png', 'workspace.png', 'skills.png', 'images.png', 'insights.png', 'gallery.png', 'chat-mobile.png', 'insights-mobile.png']) {
  if (only.size && !selectedSpecs.some((spec) => spec.file === file)) continue;
  const path = join(OUT_DIR, file);
  if (existsSync(path)) rmSync(path);
}
const server = startYahu();
let browser;
try {
  await waitForServer();
  browser = await chromium.launch({ headless: true });
  for (const spec of selectedSpecs) await capturePage(browser, spec);
} finally {
  if (browser) await browser.close();
  server.kill('SIGTERM');
}
