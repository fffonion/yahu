import { describe, expect, test } from 'bun:test';
import { DEFAULT_MOBILE_NAV_MODES, MOBILE_NAV_LIMIT, MOBILE_NAV_MODES, normalizeMobileNavModes, readMobileNavModes } from './mobileNavigation';

describe('mobile navigation selection', () => {
  test('accepts only known routes, removes duplicates, and enforces the six-item limit', () => {
    const input = ['settings', 'terminal', 'chat', 'terminal', 'cron', 'skills', 'insights', 'usage', 'images', 'memory', 'workspace'];
    const result = normalizeMobileNavModes(input);

    expect(result).toEqual(['terminal', 'chat', 'cron', 'skills', 'insights', 'usage']);
    expect(result).not.toContain('settings');
    expect(result).toHaveLength(MOBILE_NAV_LIMIT);
    expect(MOBILE_NAV_MODES).toContain('terminal');
  });

  test('allows an explicitly empty bottom menu and uses the default only when storage is absent or invalid', () => {
    expect(normalizeMobileNavModes([])).toEqual([]);
    expect(readMobileNavModes({ getItem: () => null })).toEqual([...DEFAULT_MOBILE_NAV_MODES]);
    expect(readMobileNavModes({ getItem: () => '[]' })).toEqual([]);
    expect(readMobileNavModes({ getItem: () => '{' })).toEqual([...DEFAULT_MOBILE_NAV_MODES]);
  });
});
