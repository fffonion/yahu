import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { sectionHasContent, type ProviderUsageSection } from './providerUsage';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
const routes = () => readFileSync(new URL('./hashRoute.ts', import.meta.url), 'utf8');
const backendRoutes = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');

const section = (overrides: Partial<ProviderUsageSection> = {}): ProviderUsageSection => ({
  provider: 'openrouter',
  title: 'OpenRouter',
  description: '',
  rows: [],
  windows: [],
  errors: [],
  ...overrides,
});

describe('provider usage view', () => {
  test('sectionHasContent treats rows, windows, or description as content', () => {
    expect(sectionHasContent(section())).toBe(false);
    expect(sectionHasContent(section({ rows: [{ label: 'm' }] }))).toBe(true);
    expect(sectionHasContent(section({ windows: [{ window: '5h', used: '10%' }] }))).toBe(true);
    expect(sectionHasContent(section({ description: '余额 **$1**' }))).toBe(true);
  });

  test('usage is an independent mode with its own hash route and nav buttons', () => {
    expect(routes()).toContain("| { mode: 'usage' }");
    const source = app();
    expect(source).toContain("type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'usage'");
    expect(source).toContain("mode === 'usage' && <ProviderUsageMain");
    expect(source).toContain("setNavMode('usage', true)");
    expect(css()).toContain('.provider-usage-main{');
    expect(css()).toContain('.rail-btn.nav-usage.active{');
  });

  test('view renders provider cards with windows, tables, and error notes', () => {
    const source = app();
    expect(source).toContain('function ProviderUsageMain');
    expect(source).toContain('provider-usage-card ${sectionHasContent(section)');
    expect(source).toContain('className="provider-usage-table"');
    expect(source).toContain('className="provider-usage-errors"');
    expect(source).toContain('sectionHasContent(section)');
  });

  test('frontend fetches /provider-usage and supports forced refresh', () => {
    const source = app();
    expect(source).toContain("fetch(`/provider-usage${params}`");
    expect(source).toContain("const params = force ? '?refresh=true' : '';");
    expect(source).toContain('useEffect(() => { if (mode === \'usage\') loadProviderUsage(); }');
    expect(backendRoutes()).toContain('.route("/provider-usage", get(provider_usage_handler))');
  });

  test('i18n covers the usage nav and empty/error strings', () => {
    const strings = i18n();
    expect(strings).toContain("'nav.usage'");
    expect(strings).toContain("'usage.empty'");
    expect(strings).toContain("'usage.unavailable'");
    expect(strings).toContain("'usage.refreshAria'");
  });
});
