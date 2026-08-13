import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('image browser theme integration', () => {
  test('image browser uses the host theme instead of hard-coded standalone dark colors', () => {
    const css = styles();
    expect(css).toContain('.image-browser{grid-column:2 / -1;background:var(--bg);color:var(--text)');
    expect(css).toContain('.image-toolbar{');
    expect(css).toContain('.image-toolbar{height:70px;border-bottom:1px solid var(--border);background:var(--surface);');
    expect(css).toContain('.image-grid-wrap{');
    expect(css).toContain('background:var(--bg)');

  });

  test('image browser buttons inherit themed button tokens and are not pill-shaped', () => {
    const css = styles();
    expect(css).toContain('.image-actions button,.image-overlay button,.modalbar button{border:1px solid var(--border);border-radius:var(--radius-md);');
    expect(css).toContain('background:var(--surface-2);color:var(--text)');
    expect(css).toContain('.image-actions button:hover,.image-overlay button:hover,.modalbar button:hover{background:var(--accent-soft);color:var(--accent)');

  });

  test('image cards and modal metadata keep restrained host radii', () => {
    const css = styles();
    expect(css).toContain('.image-card{position:relative;display:block;border-radius:var(--radius-card)');
    expect(css).toContain('background:var(--surface)');
    expect(css).toContain('border:1px solid var(--border)');
    expect(css).toContain('.modal-meta{box-sizing:border-box;position:fixed;z-index:118;top:22px;right:22px;bottom:78px;width:340px;');
    expect(css).toContain('border-radius:var(--radius-lg);border:1px solid var(--border);background:color-mix(in srgb,var(--surface) 94%,transparent)');
  });
});
