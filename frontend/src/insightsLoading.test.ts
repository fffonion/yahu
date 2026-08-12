import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('insights price loading', () => {
  test('bypasses stale cache and retries unpriced results with a refresh', () => {
    const source = app();
    expect(source).toContain("fetch(buildUsageUrl(force), { cache: 'no-store' })");
    expect(source).toContain("Number(nextInsights?.totals?.unpriced_tokens || 0) > 0");
    expect(source).toContain("fetch(buildUsageUrl(true), { cache: 'no-store' })");
    expect(source).toContain("Number(cached?.totals?.unpriced_tokens || 0) <= 0");
  });
});
