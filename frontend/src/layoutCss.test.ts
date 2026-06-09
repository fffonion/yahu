import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

describe('sidebar session list css', () => {
  test('session list is the shrinking scroll container inside the fixed sidebar', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.sidebar{display:grid;grid-template-columns:72px minmax(0,1fr);grid-template-rows:minmax(0,1fr) auto;border-right-width:1px;min-width:0;min-height:0;height:100vh;overflow:hidden');
    expect(css).toContain('.sessions{min-height:0;flex:1 1 auto;overflow:auto');
    expect(css).toContain('.left-body{min-height:0;min-width:0;overflow:hidden');
  });

  test('desktop composer uses a flat input bar without the old inner card frame', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.composer-wrap{position:relative;z-index:150;padding:0;background:var(--surface);box-shadow:0 -8px 22px rgba(0,0,0,.08)}');
    expect(css).toContain('.composer-box{position:relative;z-index:150;overflow:visible;width:100%;border:0;border-radius:0;background:transparent;box-shadow:none}');
    expect(css).toContain('.composer-box textarea{display:block;width:100%;height:86px;padding:15px 18px;line-height:1.5;border:0;border-radius:0;background:var(--surface);box-shadow:none;resize:none}');
    expect(css).toContain('.composer-box textarea:focus{border:0;box-shadow:none}');
    expect(css).toContain('.composer-footer{position:relative;width:100%;gap:8px;padding:10px 18px;background:var(--surface);border-top:1px solid var(--border)}');
    expect(css).not.toContain('border-radius:16px;background:var(--surface);box-shadow:0 10px 32px');
  });
});
