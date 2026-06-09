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
    expect(app).toContain('aria-label={chartTooltipLabel');
  });

  test('keeps chart axis and tooltips styled inside the chart card', () => {
    const css = cssSource();
    expect(css).toContain('.chart-y-axis{position:absolute;left:0;top:0;bottom:48px;width:52px;');
    expect(css).toContain('.chart-point-hit:hover .chart-tooltip,.chart-point-hit:focus .chart-tooltip,.chart-point-hit:focus-visible .chart-tooltip{opacity:1;transform:translate(-50%,-8px);pointer-events:auto}');
    expect(css).toContain('.usage-chart svg{width:100%;height:260px;display:block;overflow:visible;position:relative;z-index:1}');
  });
});
