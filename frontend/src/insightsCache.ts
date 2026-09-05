import type { UsageInsights } from './insights';

export type InsightsPeriod = 1 | 7 | 30;
export type InsightsSessionStorage = {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
};

const INSIGHTS_SESSION_CACHE_PREFIX = 'yahu.insights.session-cache.v1';

export function getInsightsSessionStorage(): InsightsSessionStorage | undefined {
  if (typeof window === 'undefined') return undefined;
  try {
    return window.sessionStorage;
  } catch {
    return undefined;
  }
}

export function insightsSessionCacheKey(period: InsightsPeriod, timezoneOffset: number): string {
  return `${INSIGHTS_SESSION_CACHE_PREFIX}:${Math.trunc(timezoneOffset)}:${period}`;
}

export function readInsightsSessionCache(
  storage: InsightsSessionStorage | undefined,
  period: InsightsPeriod,
  timezoneOffset: number,
): UsageInsights | null {
  if (!storage) return null;
  try {
    const raw = storage.getItem(insightsSessionCacheKey(period, timezoneOffset));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed) || Number(parsed.window_days) !== period) return null;
    return parsed as UsageInsights;
  } catch {
    return null;
  }
}

export function writeInsightsSessionCache(
  storage: InsightsSessionStorage | undefined,
  period: InsightsPeriod,
  timezoneOffset: number,
  insights: UsageInsights,
): void {
  if (!storage) return;
  try {
    storage.setItem(insightsSessionCacheKey(period, timezoneOffset), JSON.stringify(insights));
  } catch {
    // sessionStorage can be unavailable or full; the network path remains valid.
  }
}
