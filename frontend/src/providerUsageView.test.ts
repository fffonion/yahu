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
    expect(css()).toContain('.provider-usage-main{');
  });

  test('view renders independent provider cards with switches, refresh buttons, progress bars, and error notes', () => {
    const source = app();
    expect(source).toContain('function ProviderUsageMain');
    expect(source).toContain('provider-usage-provider-card insight-card');
    expect(source).toContain('provider-usage-brand-icon');
    expect(source).toContain('providerBrandFavicon');
    expect(source).not.toContain('providerBrandMark');
    expect(source).toContain('provider-usage-context-menu');
    expect(source).toContain('onContextMenu={(event) => openProviderMenu(event, provider)}');
    expect(source).toContain('rowTone(row.label)');
    expect(css()).toContain('.provider-usage-row.tone-0');
    expect(css()).toContain('.provider-usage-status-dot.is-off');
    expect(source).toContain('provider-usage-context-toggle');
    expect(source).toContain('function ProviderUsageSectionView');
    expect(source).toContain('role="progressbar"');
    expect(source).toContain('providerResetDurationText');
    expect(source).toContain('provider-usage-table');
    expect(source).toContain('ProviderUsageSkeleton');
    expect(source).not.toContain('每张卡独立查询');
    expect(source).toContain('className="provider-usage-errors"');
    expect(source).toContain('refreshProvider');
    expect(source).toContain('refreshAll');
    expect(source).toContain('usagePercentTone');
    expect(source).toContain('percent !== null &&');
    expect(source).not.toContain('const ageLabel = updatedAgo(section?.captured_at)');
    expect(source).not.toContain("t('usage.updated')");
    expect(source).toContain("section.provider === 'mimo' && win.window === '月额度'");
    expect(source).toContain('const stale = Boolean(enabled && section?.captured_at');
    expect(source).toContain("t('usage.title')");
    expect(source).toContain('function integerPercentText');
    expect(source).not.toContain('updatedAgo(props.payload?.fetched_at)');
    expect(source).not.toContain('props.payload?.fetched_at && Date.now()');
    expect(source).toContain('nav-usage');
    expect(source).toContain("setNavMode('usage', true)");
    expect(source).toContain('ChartNoAxesColumnIncreasing');
    expect(source).not.toContain('Coins');
    expect(source).toContain('is-multi-account');
    expect(source).toContain('PROVIDER_USAGE_ORDER_KEY');
    expect(source).toContain('onDragStart');
    expect(source).toContain('onDrop');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_SECONDS');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_KEY');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_INTERVAL_MS');
    expect(source).toContain('toggleProviderAutoRefresh');
    expect(source).toContain('is-auto-refreshing');
    expect(source).toContain('captured_at');
    expect(source).toContain("section.windows.filter((win) => commandcodeWindowParts(win.window)?.[1] !== '5h额度')");
    expect(source).toContain('providerBalanceText');
    expect(css()).toContain('grid-template-columns:repeat(3,minmax(0,1fr))');
    expect(css()).toContain('height:250px;min-height:250px;max-height:250px');
    expect(css()).toContain('.provider-usage-provider-card.is-multi-account');
    expect(css()).toContain('.provider-usage-progress.is-medium span');
    expect(css()).toContain('.provider-usage-title-wrap{display:flex;align-items:flex-start');
    expect(css()).toContain('.provider-usage-brand-icon{width:22px;height:22px;flex:0 0 22px;display:grid;place-items:center;margin-top:0!important;border:0');
    expect(css()).toContain('.provider-usage-provider-card.is-auto-refreshing::before');
    expect(css()).toContain('provider-usage-auto-refresh-edge 3.8s linear infinite');
    expect(css()).toContain('.provider-usage-title-row{display:flex;align-items:center;justify-content:space-between');
  });

  test('frontend loads the catalog first and refreshes providers concurrently', () => {
    const source = app();
    expect(source).toContain("fetch('/provider-usage'");
    expect(source).toContain("new URLSearchParams({ provider })");
    expect(source).toContain("params.set('refresh', 'true')");
    expect(source).toContain('await Promise.all(providers.map((provider) => loadProviderUsageProvider(provider, true)))');
    expect(source).toContain('function todayUsageIsZero');
    expect(source).toContain("return t('usage.noData')");
    expect(backendRoutes()).toContain('.route("/provider-usage", get(provider_usage_handler))');
  });

  test('i18n covers the usage nav and empty/error strings', () => {
    const strings = i18n();
    expect(strings).toContain("'nav.usage'");
    expect(strings).toContain("'usage.empty'");
    expect(strings).toContain("'usage.unavailable'");
    expect(strings).toContain("'usage.refreshAria'");
    expect(strings).toContain("'usage.refreshAllAria'");
    expect(strings).not.toContain('运营商');
  });
});
