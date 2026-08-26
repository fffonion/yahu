import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { orderProviderUsageAccountGroups, providerUsageAccountHasActiveQuotaWall, providerCodexResetSubtitle, sectionHasContent, type ProviderUsageAccountGroup, type ProviderUsageSection } from './providerUsage';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const providerUsage = () => readFileSync(new URL('./providerUsage.ts', import.meta.url), 'utf8');
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

  test('multi-account provider groups put accounts with any full quota window last', () => {
    const groups: ProviderUsageAccountGroup[] = [
      ['full-week', [{ window: 'full-week 周额度', used: '100%' }]],
      ['available', [{ window: 'available 5h额度', used: '12%' }]],
      ['full-five-hour', [{ window: 'full-five-hour 5h额度', used: '100%' }]],
      ['available-later', [{ window: 'available-later 月额度', used: '3 / 10' }]],
    ];
    expect(orderProviderUsageAccountGroups(groups).map(([account]) => account))
      .toEqual(['available', 'available-later', 'full-week', 'full-five-hour']);
  });

  test('multi-account refresh protects active quota walls and refreshes other accounts', () => {
    const now = 1_800_000_000;
    expect(providerUsageAccountHasActiveQuotaWall([
      { window: 'full 周额度', used: '100%', reset_at: now + 3600 },
      { window: 'full 5h额度', used: '12%', reset_at: now + 60 },
    ], now)).toBe(true);
    expect(providerUsageAccountHasActiveQuotaWall([
      { window: 'expired 周额度', used: '100%', reset_at: now - 1 },
    ], now)).toBe(false);
    expect(providerUsageAccountHasActiveQuotaWall([
      { window: 'refreshing 周额度', used: '80%', reset_at: now + 3600 },
    ], now)).toBe(false);
  });
  test('Codex reset subtitle keeps positive accounts and omits zero-reset accounts', () => {
    expect(providerCodexResetSubtitle('mayo：Reset：1个；到期：28天后；me：Reset：0个'))
      .toBe('mayo：1个重置 28天后到期');
    expect(providerCodexResetSubtitle('mayo：Reset：0个')).toBe('');
    expect(providerCodexResetSubtitle('mayo：Reset：2个；当前可用：1个；到期：2小时后、5天后'))
      .toBe('mayo：2个重置 2小时后、5天后到期');
  });

  test('provider toggle refreshes only the toggled provider', () => {
    const source = app();
    expect(source).toContain('if (nextEnabled) void loadProviderUsageProvider(provider, true);');
    expect(source).toContain('providerUsageEnabledRef');
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
    expect(source).not.toContain('onContextMenu={(event) => openProviderMenu(event, provider)}');
    expect(source).toContain('provider-usage-menu-trigger');
    expect(source).toContain('<MoreVertical />');
    expect(source).toContain('onPointerDown={(event) => startProviderPointer(event, provider.provider)}');
    expect(source).toContain('provider-usage-drag-placeholder');
    expect(source).toContain('providerDragRef');
    expect(source).toContain('event.dataTransfer.effectAllowed = \'move\'');
    expect(source).toContain('finishProviderDrag(providerDragRef.current?.provider || draggedProvider, provider.provider)');
    expect(css()).toContain('.provider-usage-provider-card.is-dragging');
    expect(css()).toContain('.provider-usage-provider-card.is-touch-dragging{pointer-events:none}');
    expect(css()).not.toContain('.provider-usage-provider-card.is-dragging{opacity:.58;pointer-events:none');
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
    expect(source).toContain('loading && accountCount <= 1 ? <ProviderUsageSkeleton /> : <>');
    expect(source).toContain('{section && <ProviderUsageSectionView section={section} loading={loading} />}');
    expect(source).toContain('providerUsageAccountHasActiveQuotaWall(windows)');
    expect(source).toContain('ProviderUsageAccountSkeleton');
    expect(source).toContain('const refreshing = loading && !providerUsageAccountHasActiveQuotaWall(windows);');
    expect(source).toContain('usagePercentTone');
    expect(source).toContain('progressPercent !== null &&');
    expect(source).not.toContain('const ageLabel = updatedAgo(section?.captured_at)');
    expect(source).not.toContain("t('usage.updated')");
    expect(source).toContain('integerPercentText(win.used)');
    expect(source).not.toContain('isMimoPlan');
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
    expect(source).toContain('orderProviderUsageAccountGroups');
    expect(source).toContain('const accountCount = new Set');
    expect(source).toContain('const multiAccount = accountCount > 2;');
    expect(source).toContain("['commandcode', 'codex', 'grok'].includes(section.provider)");
    expect(providerUsage()).toContain("hasQuotaWall: windows.some");
    expect(source).toContain('PROVIDER_USAGE_ORDER_KEY');
    expect(source).toContain('onDragStart');
    expect(source).toContain('onDrop');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_SECONDS');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_KEY');
    expect(source).toContain('PROVIDER_USAGE_AUTO_REFRESH_INTERVAL_MS');
    expect(source).toContain('toggleProviderAutoRefresh');
    expect(source).toContain('is-auto-refreshing');
    expect(source).toContain('captured_at');
    expect(source).toContain("windows.filter((win) => providerAccountWindowParts(win.window)?.[1] !== '5h额度')");
    expect(source).toContain('providerBalanceText');
    expect(css()).toContain('grid-template-columns:repeat(3,minmax(0,1fr))');
    expect(css()).toContain('height:250px;min-height:250px;max-height:250px');
    expect(css()).toContain('.provider-usage-provider-card.is-multi-account');
    expect(css()).toContain('.provider-usage-progress.is-medium span');
    expect(css()).toContain('.provider-usage-window-skeleton');
    expect(css()).toContain('.provider-usage-account-group.is-loading');
    expect(css()).toContain('.provider-usage-title-wrap{display:flex;align-items:flex-start');
    expect(css()).toContain('.provider-usage-brand-icon{width:22px;height:22px;flex:0 0 22px;display:grid;place-items:center;margin-top:0!important;border:0');
    expect(css()).toContain('.provider-usage-provider-card.is-auto-refreshing::before');
    expect(css()).toContain('provider-usage-auto-refresh-edge 7.6s linear infinite');
    expect(css()).toContain('.provider-usage-title-row{display:flex;align-items:center;justify-content:space-between');
  });

  test('mobile provider long press owns pointer movement and auto-scrolls at screen edges', () => {
    const source = app();
    expect(source).toContain('const providerUsageBodyRef = useRef<HTMLElement | null>(null);');
    expect(source).toContain('targetElement.setPointerCapture(event.pointerId)');
    expect(source).toContain("targetElement.style.touchAction = 'none'");
    expect(source).toContain('scrollProviderByFinger(event.clientY - press.startY)');
    expect(source).toContain('window.requestAnimationFrame(runProviderAutoScroll)');
    expect(source).toContain('element.scrollTop + delta');
    expect(source).toContain('updateProviderDragTarget(press.lastX, press.lastY, press.provider)');
    const styles = css();
    const panYIndex = styles.lastIndexOf('.provider-usage-provider-card{touch-action:pan-y;user-select:none}');
    const mobileNoneIndex = styles.lastIndexOf('@media (max-width:760px){.provider-usage-provider-card{touch-action:none}}');
    expect(panYIndex).toBeGreaterThanOrEqual(0);
    expect(mobileNoneIndex).toBeGreaterThan(panYIndex);
  });

  test('frontend loads the catalog first and refreshes providers concurrently', () => {
    const source = app();
    expect(source).toContain("fetch('/provider-usage'");
    expect(source).toContain("new URLSearchParams({ provider })");
    expect(source).toContain("params.set('refresh', 'true')");
    expect(source).toContain('await Promise.all(providers.map((provider) => loadProviderUsageProvider(provider, true)))');
    expect(source).toContain('function todayUsageIsZero');
    expect(source).not.toContain("t('usage.noData')");
    expect(source).not.toContain("t('usage.noWindowData')");
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
