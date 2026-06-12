import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('insights chart UI', () => {
  test('renders left value axis and hoverable datapoint tooltips', () => {
    const app = appSource();
    expect(app).toContain('className="chart-y-axis"');
    expect(app).toContain('className={`chart-point-hit tooltip-${tooltipPlacement} tooltip-align-${tooltipAlign}`}');
    expect(app).toContain('className="chart-tooltip"');
    expect(app).toContain('aria-label={label}');
  });

  test('uses no-decimal compact y-axis token labels on narrow screens', () => {
    const app = appSource();
    expect(app).toContain("fmtCompactAxisTick");
    expect(app).toContain("useMediaQuery('(max-width: 760px)')");
    expect(app).toContain("compactAxisLabels ? fmtCompactAxisTick(value) : formatMetricValue(metric, value)");
  });

  test('hides datapoint dots until hover or keyboard focus', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).not.toContain('className="usage-dot"');
    expect(css).not.toContain('.usage-dot{');
    expect(css).toContain('.chart-points{position:absolute;left:0;right:0;top:0;height:260px;z-index:4;pointer-events:auto}');
    expect(app).toContain('const pointHitWidthPct = buckets.length > 1 ? (((width - pad.left - pad.right) / (buckets.length - 1)) * 0.8 / width) * 100 : (24 / width) * 100;');
    expect(app).toContain("'--hit-width': `${pointHitWidthPct}%`");
    expect(css).toContain('.chart-point-hit{position:absolute;display:block;width:var(--hit-width,24px);height:24px;margin:-12px 0 0 calc(var(--hit-width,24px) * -.5);border-radius:999px;pointer-events:auto;cursor:crosshair}');
    expect(css).toContain('.chart-point-hit::after{content:"";position:absolute;width:6px;height:6px;left:50%;top:50%;margin:-3px 0 0 -3px;border-radius:999px;background:var(--point-color,var(--accent));box-shadow:0 0 0 2px color-mix(in srgb,var(--surface) 80%,transparent);opacity:0;');
    expect(css).toContain('.chart-point-hit:hover::after,.chart-point-hit:focus-visible::after{opacity:1;transform:scale(1.12)}');
  });

  test('keeps chart axis and tooltips styled inside the chart card', () => {
    const css = cssSource();
    expect(css).toContain('.chart-y-axis{position:absolute;left:0;top:0;bottom:48px;width:52px;');
    expect(css).toContain('.chart-point-hit:hover .chart-tooltip,.chart-point-hit:focus .chart-tooltip,.chart-point-hit:focus-visible .chart-tooltip{opacity:1;transform:translate(-50%,-8px);pointer-events:auto}');
    expect(css).toContain('.chart-point-hit.tooltip-below .chart-tooltip{top:22px;bottom:auto}');
    expect(css).toContain('.chart-point-hit.tooltip-align-start .chart-tooltip{left:50%;transform:translate(0,0)}');
    expect(css).toContain('.chart-point-hit.tooltip-align-end .chart-tooltip{left:auto;right:50%;transform:translate(0,0)}');
    expect(css).toContain('.chart-point-hit.tooltip-below:hover .chart-tooltip,.chart-point-hit.tooltip-below:focus .chart-tooltip,.chart-point-hit.tooltip-below:focus-visible .chart-tooltip{transform:translate(-50%,8px)}');
    expect(css).toContain('.chart-axis{position:absolute;left:var(--insights-plot-left);right:var(--insights-plot-right);bottom:32px;height:18px;color:var(--muted);font-size:11px}');
  });

  test('renders dedicated loading placeholders for cards chart and model rows', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const showSkeleton = props.loading');
    expect(app).toContain("<InsightCardSkeleton label={t('insights.tokens')} />");
    expect(app).toContain('<UsageChartSkeleton />');
    expect(app).toContain('<ModelUsageSkeletonList />');
    expect(css).toContain('.skeleton-number{width:min(78%,190px);height:31px}');
    expect(css).toContain('.usage-chart-loading{min-height:260px;display:grid;place-items:center;');
    expect(css).toContain('.model-skeleton-list{display:grid;gap:10px}');
  });

  test('renders cost metric option and per-model USD cost sublabel without FX calls', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain("usageMetricLabel('cost_usd')");
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

  test('chart header uses an icon stack toggle button and stacked mode draws model fills under one total line', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const [chartStacked, setChartStacked] = useState(false)');
    expect(app).toContain('className="chart-stack-toggle icon-btn"');
    expect(app).toContain("aria-label={chartStacked ? t('insights.showUnstackedChart') : t('insights.showStackedChart')}");
    expect(app).toContain('{chartStacked ? <LineChart /> : <Layers />}');
    expect(app).not.toContain("{chartStacked ? 'Unstack' : 'Stack'}");
    expect(app).toContain('aria-pressed={chartStacked}');
    expect(app).toContain('<UsageAreaChart buckets={isSingleDay ? activeHours : activeDays} models={models} metric={props.metric} stacked={chartStacked} />');
    expect(app).toContain('className={`usage-chart ${stacked ? \'stacked\' : \'unstacked\'}`}');
    expect(app).toContain('isHourlyBucket(buckets[0]) ? modelHourlyMetricValues(model, buckets as UsageHour[], metric) : modelDailyMetricValues(model, buckets as UsageDay[], metric)');
    expect(app).toContain('className="usage-stack-area"');
    expect(app).toContain('className="usage-total-line"');
    expect(app).not.toContain('<LineChart /></div>');
    expect(css).toContain('.chart-stack-toggle{width:34px;height:34px;padding:0;');
    expect(css).toContain('.usage-chart.stacked .usage-line{display:none}');
    expect(css).toContain('.usage-total-line{fill:none;stroke:var(--accent);');
  });

  test('uses half-width strokes for desktop mobile and stacked total lines', () => {
    const css = cssSource();
    expect(css).toContain('.usage-line{fill:none;stroke-width:.4;');
    expect(css).toContain('.usage-line{stroke-width:.375}');
    expect(css).toContain('.usage-total-line{fill:none;stroke:var(--accent);stroke-width:.425;');
  });

  test('renders one-day usage as an hourly chart with stack and fill, and embeds the share bar under the chart', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const isSingleDay = props.period === 1');
    expect(app).toContain('const activeHours = props.insights?.hourly || []');
    expect(app).toContain('<button type="button" className="chart-stack-toggle icon-btn"');
    expect(app).not.toContain('{!isSingleDay && <button type="button" className="chart-stack-toggle icon-btn"');
    expect(app).not.toContain('fillArea={false}');
    expect(app).not.toContain('usage-total-hour-line');
    expect(app).toContain('function UsageShareBar');
    expect(app).not.toContain('<section className="insights-chart-card insights-share-card">');
    expect(app).toContain('<UsageAreaChart buckets={isSingleDay ? activeHours : activeDays} models={models} metric={props.metric} stacked={chartStacked} />');
    expect(app).toContain('<UsageShareBar models={models} metric={props.metric} />');
    expect(app).toContain('className="usage-share-map"');
    expect(app).toContain('className="usage-share-bar"');
    expect(app).toContain('className="usage-share-segment"');
    expect(app).toContain('className="usage-share-indicators"');
    expect(app).toContain('style={{ width: `${item.pct}%`, background: `var(--chart-${item.index})` }}');
    expect(app).toContain('className="usage-share-indicator"');
    expect(app).toContain("'--share-start': `${item.start}%`");
    expect(css).toContain('.insights-main{grid-column:2 / -1;--chart-0:#14b8a6;--chart-1:#8b5cf6;--chart-2:#f59e0b;--chart-3:#38bdf8;--chart-4:#ec4899;--chart-5:#84cc16;--insights-plot-left:8.0556%;--insights-plot-right:2.5%}');
    expect(css).toContain('.usage-share-chart{display:grid;gap:0;min-height:64px;align-content:center;padding:6px 0 0}');
    expect(css).toContain('.usage-share-map{display:grid;grid-template-rows:auto 42px;gap:9px;min-width:0;margin-left:var(--insights-plot-left);margin-right:var(--insights-plot-right)}');
    expect(css).toContain('.usage-share-bar{height:9px;border:1px solid var(--border);border-radius:999px;overflow:hidden;display:flex;');
    expect(css).toContain('.usage-share-indicators{display:flex;flex-wrap:wrap;gap:6px 12px;min-width:0;max-width:100%}');
    expect(css).toContain('.usage-share-indicator{position:relative;max-width:min(220px,100%);min-width:0;display:inline-grid;');
    expect(css).toContain('.usage-share-indicator::before,.usage-share-indicator::after{display:none}');
    expect(app).not.toContain('className="chart-legend"');
  });

  test('other signal sources are selected from the active period totals', () => {
    const app = appSource();
    expect(app).toContain('const activeSources = periodSources(props.insights?.periods || [], props.insights?.sources || [], props.period);');
    expect(app).toContain('<SourceSignalList sources={activeSources.slice(0, 6)} />');
    expect(app).not.toContain('<SourceSignalList sources={(props.insights?.sources || []).slice(0, 6)} />');
  });

  test('keeps metric card glow away from rounded corners in light themes', () => {
    const css = cssSource();
    expect(css).toContain('.insight-card::after{content:"";position:absolute;inset:auto 14px -38px 14px;');
    expect(css).toContain('height:72px;border-radius:999px;background:linear-gradient(90deg,color-mix(in srgb,var(--accent) 22%,transparent),color-mix(in srgb,var(--accent-2) 16%,transparent));');
    expect(css).not.toContain('inset:auto -30% -55% -30%');
  });

  test('centers insights refresh icon inside desktop and mobile header buttons', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('className="icon-btn mobile-icon-only insights-refresh"');
    expect(css).toContain('.header-actions .icon-btn{height:38px;min-width:38px;padding:0;justify-content:center}');
    expect(css).toContain('.header-actions .icon-btn svg{display:block;flex:0 0 auto}');
    expect(css).toContain('.header-actions .insights-refresh{width:38px}');
    expect(css).toContain('.header-actions .insights-refresh{width:30px;height:30px;min-width:30px;padding:0}');
    expect(css).toContain('.usage-share-map{grid-template-rows:auto auto;gap:7px;margin-left:var(--insights-plot-left);margin-right:var(--insights-plot-right)}');
    expect(css).toContain('.chart-axis{left:var(--insights-plot-left);right:var(--insights-plot-right)}');
    expect(css).not.toContain('.chart-axis{left:52px;right:16px}');
  });
});
