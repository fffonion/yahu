import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('insights chart UI', () => {
  test('renders left value axis and hoverable datapoint tooltips', () => {
    const app = appSource();
    expect(app).toContain('className="chart-y-axis"');
    expect(app).toContain('className="chart-point-hit"');
    expect(app).toContain('className="chart-tooltip"');
    expect(app).toContain('aria-label={label}');
  });

  test('hides datapoint dots until hover or keyboard focus', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).not.toContain('className="usage-dot"');
    expect(css).not.toContain('.usage-dot{');
    expect(css).toContain('.chart-points{position:absolute;left:0;right:0;top:0;height:260px;z-index:4;pointer-events:auto}');
    expect(css).toContain('.chart-point-hit{position:absolute;display:block;width:24px;height:24px;margin:-12px 0 0 -12px;border-radius:999px;pointer-events:auto;cursor:crosshair}');
    expect(css).toContain('.chart-point-hit::after{content:"";position:absolute;inset:9px;border-radius:999px;background:var(--point-color,var(--accent));box-shadow:0 0 0 2px color-mix(in srgb,var(--surface) 80%,transparent);opacity:0;');
    expect(css).toContain('.chart-point-hit:hover::after,.chart-point-hit:focus-visible::after{opacity:1;transform:scale(1.12)}');
  });

  test('keeps chart axis and tooltips styled inside the chart card', () => {
    const css = cssSource();
    expect(css).toContain('.chart-y-axis{position:absolute;left:0;top:0;bottom:48px;width:52px;');
    expect(css).toContain('.chart-point-hit:hover .chart-tooltip,.chart-point-hit:focus .chart-tooltip,.chart-point-hit:focus-visible .chart-tooltip{opacity:1;transform:translate(-50%,-8px);pointer-events:auto}');
    expect(css).toContain('.usage-chart svg{width:100%;height:260px;display:block;overflow:visible;position:relative;z-index:1}');
  });

  test('renders dedicated loading placeholders for cards chart and model rows', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const showSkeleton = props.loading');
    expect(app).toContain('<InsightCardSkeleton label="Tokens" />');
    expect(app).toContain('<UsageChartSkeleton />');
    expect(app).toContain('<ModelUsageSkeletonList />');
    expect(css).toContain('.skeleton-number{width:min(78%,190px);height:31px}');
    expect(css).toContain('.usage-chart-loading{min-height:260px;display:grid;place-items:center;');
    expect(css).toContain('.model-skeleton-list{display:grid;gap:10px}');
  });

  test('renders cost metric option and per-model USD cost sublabel without FX calls', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('metricLabels.cost_usd');
    expect(app).toContain('className="model-value"');
    expect(app).toContain('className="model-cost-sub"');
    expect(app).not.toContain('/insights/fx');
    expect(app).not.toContain('currencyRates');
    expect(css).toContain('.model-value{display:grid;justify-items:end;gap:3px}');
    expect(css).toContain('.model-cost-sub{font-size:11px;color:var(--muted);line-height:1}');
  });

  test('renders insights source channels as a wrapping right-panel list instead of one compressed line', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('function SourceSignalList');
    expect(app).toContain('className="signal-row signal-row-sources"');
    expect(app).toContain('className="source-channel-list"');
    expect(app).toContain('className="source-channel-chip"');
    expect(app).not.toContain("<SignalRow name=\"Sources\" value={(props.insights?.sources || []).slice(0, 3).map((item) => `${item.source} ${fmtTokens(item.totals.total_tokens)}`).join(' · ') || '—'} />");
    expect(css).toContain('.signal-row-sources{display:grid;grid-template-columns:1fr;gap:8px}');
    expect(css).toContain('.source-channel-list{display:grid;grid-template-columns:1fr;gap:6px;min-width:0}');
    expect(css).toContain('.source-channel-chip{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:8px;max-width:100%;');
  });

  test('keeps metric card glow away from rounded corners in light themes', () => {
    const css = cssSource();
    expect(css).toContain('.insight-card::after{content:"";position:absolute;inset:auto 14px -38px 14px;');
    expect(css).toContain('height:72px;border-radius:999px;background:linear-gradient(90deg,color-mix(in srgb,var(--accent) 22%,transparent),color-mix(in srgb,var(--accent-2) 16%,transparent));');
    expect(css).not.toContain('inset:auto -30% -55% -30%');
  });
});
