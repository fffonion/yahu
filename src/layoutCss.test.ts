import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

describe('sidebar session list css', () => {
  test('session list is the shrinking scroll container inside the fixed sidebar', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.sidebar{display:grid;grid-template-columns:72px minmax(0,1fr);grid-template-rows:minmax(0,1fr) auto;border-right-width:1px;min-width:0;min-height:0;height:100vh;overflow:hidden');
    expect(css).toContain('.sessions{min-height:0;flex:1 1 auto;overflow:auto');
    expect(css).toContain('.left-body{min-height:0;min-width:0;overflow:hidden');
  });
});
