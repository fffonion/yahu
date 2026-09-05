import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('insights price loading', () => {
  test('caches unpriced results and refreshes only when explicitly requested', () => {
    const source = app();
    expect(source).toContain("fetch(buildUsageUrl(force), { cache: 'no-store' })");
    expect(source).toContain('writeInsightsSessionCache(sessionStorage, period, timezoneOffset, nextInsights);');
    expect(source).toContain('if (cached && !force) {');
    expect(source).not.toContain("Number(nextInsights?.totals?.unpriced_tokens || 0) > 0");
    expect(source).not.toContain("fetch(buildUsageUrl(true), { cache: 'no-store' })");
    expect(source).not.toContain("Number(cached?.totals?.unpriced_tokens || 0) <= 0");
  });
});
