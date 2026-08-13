import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('insights chart plot gutter', () => {
  test('uses a tighter y-axis label gutter while keeping plot region and share bar aligned', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const pad = { top: 14, right: 18, bottom: 28, left: 30 };');
    expect(css).toContain('--insights-plot-left:4.1667%;--insights-plot-right:2.5%');
    expect(css).toContain('.chart-y-axis{position:absolute;left:16px;top:0;bottom:48px;width:30px;');
    expect(css).toContain('.chart-y-axis span{position:absolute;right:0;');

    expect(css).toContain('.usage-share-map{display:grid;grid-template-rows:auto 42px;gap:9px;min-width:0;margin-left:var(--insights-plot-left);margin-right:var(--insights-plot-right)}');
    expect(css).toContain('.chart-axis{position:absolute;left:var(--insights-plot-left);right:var(--insights-plot-right);bottom:32px;height:18px;color:var(--muted);font-size:11px}');
  });

  test('mobile y-axis shifts right without moving the chart plot', () => {
    const css = cssSource();
    expect(css).toContain('.chart-y-axis{left:8px;bottom:46px;width:28px;font-size:10px}.chart-y-axis span{right:0}');
    expect(css).toContain('.chart-axis{left:var(--insights-plot-left);right:var(--insights-plot-right)}');

  });
});
