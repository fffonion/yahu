import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

describe('sidebar session list css', () => {
  const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

  test('session list is the shrinking scroll container inside the fixed sidebar', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.sidebar{display:grid;grid-template-columns:72px minmax(0,1fr);grid-template-rows:minmax(0,1fr) auto;border-width:0;border-right-width:1px;min-width:0;min-height:0;height:100vh;overflow:hidden');
    expect(css).toContain('.sessions{min-height:0;flex:1 1 auto;overflow:auto');
    expect(css).toContain('.left-body{min-height:0;min-width:0;overflow:hidden');
  });

  test('insights period switches keep a reserved scroll gutter', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.insights-content{min-height:0;overflow-x:hidden;overflow-y:scroll;scrollbar-gutter:stable;');
    expect(css).toContain('.insights-content{padding:10px 10px calc(86px + env(safe-area-inset-bottom,0px));gap:10px;grid-template-rows:auto auto auto auto;overflow-x:hidden;overflow-y:auto;scrollbar-gutter:stable;');
  });

  test('desktop composer uses a flat input bar without the old inner card frame', () => {
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(css).toContain('.composer-wrap{position:relative;z-index:150;padding:0;background:var(--surface);box-shadow:0 -8px 22px rgba(0,0,0,.08)}');
    expect(css).toContain('.composer-box{position:relative;z-index:150;overflow:visible;width:100%;border:0;border-radius:0;background:transparent;box-shadow:none}');
    expect(css).toContain('.composer-box textarea{display:block;width:100%;height:96px;min-height:96px;max-height:20dvh;padding:14px 18px 64px;line-height:1.5;border:0;border-radius:0;background:var(--surface);box-shadow:none;resize:none;overflow-y:hidden}');
    expect(css).toContain('.composer-box textarea{padding:5px 18px 64px}');
    expect(css).toContain('.composer-box textarea:focus{border:0;box-shadow:none}');
    expect(css).toContain('.composer-footer{position:absolute;left:0;right:0;bottom:0;width:100%;gap:8px;padding:10px 18px;background:transparent;border-top:0}');
    expect(css).not.toContain('border-radius:16px;background:var(--surface);box-shadow:0 10px 32px');
  });

  test('composer textarea grows to 20 percent of the viewport before scrolling', () => {
    const source = app();
    const css = readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
    expect(source).toContain('const resizeComposerTextarea = useCallback(() =>');
    expect(source).toContain('const maxHeight = Math.max(minHeight, Math.floor(window.innerHeight * 0.2));');
    expect(source).toContain("textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden'");
    expect(source).toContain('ref={textareaRef}');
    expect(css).toContain('max-height:20dvh');
    expect(css).toContain('padding:14px 18px 64px');
  });
});
