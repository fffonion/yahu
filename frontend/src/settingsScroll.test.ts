import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('settings scroll layout', () => {
  test('desktop settings content is the scroll container inside the fixed-height main panel', () => {
    const styles = css();
    expect(styles).toContain('.main-panel{min-width:0;height:var(--app-viewport-height,100dvh);background:var(--bg);display:grid;grid-template-columns:minmax(0,1fr);grid-template-rows:auto minmax(0,1fr) auto;overflow:hidden}');
    expect(styles).toContain('.settings-content{padding:24px;display:grid;gap:16px;align-content:start;width:100%;max-width:none;box-sizing:border-box;min-width:0;min-height:0;overflow:auto;overscroll-behavior:contain}');
    expect(styles).not.toContain('.settings-content{padding:24px;display:grid;gap:16px;align-content:start;max-width:760px');
  });
});
