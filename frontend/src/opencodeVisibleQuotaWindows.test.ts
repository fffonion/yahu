import { describe, expect, test } from 'bun:test';
import { opencodeVisibleQuotaWindows as select, type ProviderUsageWindow } from './providerUsage';
const win = (window: string, used?: string | null): ProviderUsageWindow => ({ window, used, reset: '5小时', reset_at: 2000000000 });

describe('OpenCode visible quota windows', () => {
  test('keeps the higher short-window percentage plus monthly without mutating the source', () => {
    for (const prefix of ['', 'Account One ']) {
      for (const [five, week, label] of [['61%', '20%', '5h额度'], ['12%', '72%', '周额度'], ['50%', '50%', '周额度'], ['9.9%', '10%', '周额度'], ['9/10', '80%', '5h额度']]) {
        const windows = [win(prefix + '5h额度', five), win(prefix + '周额度', week), win(prefix + '月额度', '0%')];
        const before = JSON.stringify(windows);
        const actual = select(windows);
        expect(actual.map(w => w.window)).toEqual([prefix + label, prefix + '月额度']);
        expect(actual[1]).toBe(windows[2]);
        expect(actual[0]).toBe(windows[label === '5h额度' ? 0 : 1]);
        expect(JSON.stringify(windows)).toBe(before);
      }
    }
  });
  test('retains monthly with missing short windows and prefers known percentages', () => {
    const monthly = win('月额度', '0%');
    expect(select([monthly])).toEqual([monthly]);
    expect(select([])).toEqual([]);
    expect(select([win('5h额度', null), win('周额度', '0%'), monthly]).map(w => w.window)).toEqual(['周额度', '月额度']);
    expect(select([win('5h额度', '0%'), win('周额度', '-'), monthly]).map(w => w.window)).toEqual(['5h额度', '月额度']);
    expect(select([win('5h额度', null), monthly]).map(w => w.window)).toEqual(['5h额度', '月额度']);
  });
});
