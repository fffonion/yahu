import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { DEFAULT_MOBILE_NAV_MODES, MOBILE_NAV_LIMIT, MOBILE_NAV_MODES, normalizeMobileNavModes, readMobileNavModes } from './mobileNavigation';

const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('mobile navigation selection', () => {
  test('accepts only known routes, removes duplicates, and enforces the seven-item limit', () => {
    const input = ['settings', 'terminal', 'chat', 'terminal', 'cron', 'skills', 'insights', 'usage', 'workspace', 'images', 'memory'];
    const result = normalizeMobileNavModes(input);

    expect(result).toEqual(['terminal', 'chat', 'cron', 'skills', 'insights', 'usage', 'workspace']);
    expect(result).not.toContain('settings');
    expect(result).toHaveLength(MOBILE_NAV_LIMIT);
    expect(MOBILE_NAV_LIMIT).toBe(7);
    expect(MOBILE_NAV_MODES).toContain('terminal');
  });

  test('includes workspace in the default menu and migrates the previous default menu', () => {
    expect(DEFAULT_MOBILE_NAV_MODES).toContain('workspace');
    const legacyDefault = JSON.stringify(['chat', 'cron', 'skills', 'insights', 'usage', 'terminal']);
    const storage = { getItem: (key: string) => key === 'yahu.mobile-nav.v1' ? legacyDefault : null };
    expect(readMobileNavModes(storage)).toEqual([...DEFAULT_MOBILE_NAV_MODES]);
  });

  test('keeps workspace visible in the mobile bottom menu while settings stays header-only', () => {
    const source = styles();
    expect(source).toContain('.mobile-bottom-nav .nav-settings{display:none!important}');
    expect(source).not.toContain('.mobile-bottom-nav .nav-workspace,.mobile-bottom-nav .nav-settings{display:none!important}');
  });

  test('allows an explicitly empty bottom menu and uses the default only when storage is absent or invalid', () => {
    expect(normalizeMobileNavModes([])).toEqual([]);
    expect(readMobileNavModes({ getItem: () => null })).toEqual([...DEFAULT_MOBILE_NAV_MODES]);
    expect(readMobileNavModes({ getItem: () => '[]' })).toEqual([]);
    expect(readMobileNavModes({ getItem: () => '{' })).toEqual([...DEFAULT_MOBILE_NAV_MODES]);
  });
});
