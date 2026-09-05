import { describe, expect, test } from 'bun:test';
import type { UsageInsights } from './insights';
import { insightsSessionCacheKey, readInsightsSessionCache, writeInsightsSessionCache } from './insightsCache';

type MemoryStorage = {
  values: Map<string, string>;
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
};

function memoryStorage(): MemoryStorage {
  const values = new Map<string, string>();
  return {
    values,
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => { values.set(key, value); },
  };
}

function sampleInsights(windowDays: 1 | 7 | 30): UsageInsights {
  return { object: 'usage', generated_at: 123, window_days: windowDays, totals: {} as UsageInsights['totals'], daily: [], models: [], sources: [], periods: [] };
}

describe('Insights session cache', () => {
  test('stores and restores a period for the current timezone', () => {
    const storage = memoryStorage();
    const insights = sampleInsights(7);

    writeInsightsSessionCache(storage, 7, -480, insights);

    expect(storage.values.has(insightsSessionCacheKey(7, -480))).toBe(true);
    expect(readInsightsSessionCache(storage, 7, -480)).toEqual(insights);
  });

  test('keeps periods and timezones isolated', () => {
    const storage = memoryStorage();
    const sevenDays = sampleInsights(7);
    const oneDay = sampleInsights(1);

    writeInsightsSessionCache(storage, 7, -480, sevenDays);
    writeInsightsSessionCache(storage, 1, -480, oneDay);

    expect(readInsightsSessionCache(storage, 7, -480)).toEqual(sevenDays);
    expect(readInsightsSessionCache(storage, 1, -480)).toEqual(oneDay);
    expect(readInsightsSessionCache(storage, 7, 0)).toBeNull();
  });

  test('ignores malformed or unavailable session storage', () => {
    const storage = memoryStorage();
    storage.values.set(insightsSessionCacheKey(7, -480), '{bad json');

    expect(readInsightsSessionCache(storage, 7, -480)).toBeNull();
    expect(() => writeInsightsSessionCache({ getItem: () => null, setItem: () => { throw new Error('quota'); } }, 7, -480, sampleInsights(7))).not.toThrow();
  });
});
