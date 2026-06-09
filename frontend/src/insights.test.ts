import { describe, expect, test } from 'bun:test';
import { areaPath, emptyTotals, finalizeTotals, fmtPercent, fmtTokens, linePath, modelPeriodTotals, type UsageModel } from './insights';

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
});
