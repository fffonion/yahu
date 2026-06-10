import { describe, expect, test } from 'bun:test';
import { areaPath, chartPoint, chartTooltipLabel, chartYAxisTicks, emptyTotals, finalizeTotals, fmtMoney, fmtPercent, fmtTokens, linePath, metricLabels, modelPeriodTotals, type UsageModel } from './insights';

describe('insights helpers', () => {
  test('formats token and percent metrics compactly', () => {
    expect(fmtTokens(1532)).toBe('1.5K');
    expect(fmtTokens(2_400_000)).toBe('2.4M');
    expect(fmtPercent(0.941)).toBe('94%');
  });

  test('builds animated svg line and gradient area paths from usage values', () => {
    expect(linePath([0, 50, 100], 300, 120)).toContain('L');
    const area = areaPath([0, 50, 100], 300, 120);
    expect(area.endsWith('Z')).toBe(true);
    expect(area).toContain('L 288.00 108.00');
  });

  test('uses left-side axis padding and common max for chart coordinates', () => {
    const point = chartPoint(1, 50, 3, 300, 120, { top: 10, right: 20, bottom: 30, left: 60 }, 100);
    expect(point.x).toBeCloseTo(170, 2);
    expect(point.y).toBeCloseTo(50, 2);
    expect(linePath([0, 50, 100], 300, 120, { top: 10, right: 20, bottom: 30, left: 60 }, 200)).toContain('L 170.00 70.00');
  });

  test('creates readable y-axis ticks and point tooltip labels', () => {
    const ticks = chartYAxisTicks([0, 1200, 2400], 3);
    expect(ticks).toEqual([
      { value: 2400, label: '2.4K', pct: 0 },
      { value: 1200, label: '1.2K', pct: 50 },
      { value: 0, label: '0', pct: 100 },
    ]);
    expect(chartTooltipLabel('minimax-m3', '06/09', 1532, 'Input')).toBe('minimax-m3 · 06/09 · Input 1.5K');
  });

  test('summarizes a model over the selected period and recomputes derived metrics', () => {
    const model: UsageModel = {
      model: 'minimax-m3',
      totals: emptyTotals(),
      daily: [
        { date: '2026-06-07', label: '06/07', totals: { ...emptyTotals(), sessions: 1, input: 100, cache_read: 900, total_tokens: 1000 } },
        { date: '2026-06-08', label: '06/08', totals: { ...emptyTotals(), sessions: 1, output: 50, total_tokens: 50 } },
      ],
    };
    const totals = finalizeTotals(modelPeriodTotals(model, 2));
    expect(totals.total_tokens).toBe(1050);
    expect(totals.sessions).toBe(2);
    expect(Math.round(totals.cache_hit_rate * 100)).toBe(90);
  });

  test('rolls up row-preferred cost and unpriced token counters', () => {
    const model: UsageModel = {
      model: 'minimax-m3',
      totals: emptyTotals(),
      daily: [
        { date: '2026-06-07', label: '06/07', totals: { ...emptyTotals(), sessions: 1, total_tokens: 100, estimated_cost_usd: 0.1, cost_usd: 0.1 } },
        { date: '2026-06-08', label: '06/08', totals: { ...emptyTotals(), sessions: 1, total_tokens: 50, actual_cost_usd: 0.2, cost_usd: 0.2, unpriced_tokens: 10 } },
      ],
    };
    const totals = finalizeTotals(modelPeriodTotals(model, 2));
    expect(totals.cost_usd).toBeCloseTo(0.3, 6);
    expect(totals.unpriced_tokens).toBe(10);
  });

  test('includes cost as a chart metric option', () => {
    expect(metricLabels.cost_usd).toBe('Cost');
  });

  test('formats costs as USD regardless of language or exchange-rate data', () => {
    const rates = { USD: 1, CNY: 7.2, JPY: 155 };
    expect(fmtMoney(1.25)).toBe('$1.25');
    expect(fmtMoney(1.25, 'zh-CN', rates)).toBe('$1.25');
    expect(fmtMoney(1.25, 'ja', rates)).toBe('$1.25');
    expect(fmtMoney(0.001, 'zh-CN', rates)).toBe('$0.0010');
  });
});
