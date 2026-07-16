import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';

const helperUrl = new URL('./sidebarWidth.ts', import.meta.url);
const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('resizable shared left sidebar', () => {
  test('clamps persisted, pointer, and keyboard widths to the desktop range', async () => {
    expect(existsSync(helperUrl)).toBe(true);
    const { readSidebarWidth, sidebarWidthFromPointer, sidebarWidthFromKey } = await import('./sidebarWidth');

    expect(readSidebarWidth(null)).toBe(360);
    expect(readSidebarWidth('oops')).toBe(360);
    expect(readSidebarWidth('100')).toBe(260);
    expect(readSidebarWidth('1000')).toBe(560);
    expect(sidebarWidthFromPointer(440, 80)).toBe(360);
    expect(sidebarWidthFromKey(360, 'ArrowLeft')).toBe(344);
    expect(sidebarWidthFromKey(360, 'ArrowRight')).toBe(376);
    expect(sidebarWidthFromKey(360, 'Home')).toBe(260);
    expect(sidebarWidthFromKey(360, 'End')).toBe(560);
    expect(sidebarWidthFromKey(360, 'Enter')).toBe(360);
  });

  test('uses one shared persisted width and resize separator across sidebar pages', () => {
    const source = app();
    expect(source).toContain("const SIDEBAR_WIDTH_KEY = 'sidebarWidth';");
    expect(source).toContain('const [sidebarWidth, setSidebarWidth] = useState(() => readSidebarWidth(localStorage.getItem(SIDEBAR_WIDTH_KEY)));');
    expect(source).toContain("style={{ '--sidebar-width': `${sidebarWidth}px` } as React.CSSProperties}");
    expect(source).toContain('className="sidebar-resize-handle"');
    expect(source).toContain("event.currentTarget.closest('.app-shell')");
    expect(source).toContain('role="separator"');
    expect(source).toContain("aria-orientation=\"vertical\"");
    expect(source).toContain("localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidthRef.current));");
    expect(source).toContain("localStorage.setItem(SIDEBAR_WIDTH_KEY, String(nextWidth));");
  });

  test('drives every expanded desktop layout from the same width and hides the handle when unavailable', () => {
    const styles = css();
    expect(styles).toContain('.app-shell{--sidebar-width:360px;display:grid;grid-template-columns:var(--sidebar-width) minmax(520px,1fr) 320px;');
    expect(styles).toContain('.app-shell.wide-mode,.app-shell.image-mode{grid-template-columns:var(--sidebar-width) minmax(0,1fr)}');
    expect(styles).toContain('.app-shell.skills-mode{grid-template-columns:var(--sidebar-width) minmax(480px,1fr) 320px}');
    expect(styles).toContain('.sidebar-resize-handle{position:absolute;z-index:20;top:0;left:calc(var(--sidebar-width) - 4px);bottom:0;width:8px;cursor:col-resize;touch-action:none');
    expect(styles).toContain('.app-shell.nav-collapsed .sidebar-resize-handle{display:none}');
    expect(styles).toContain('@media(max-width:900px){.sidebar-resize-handle{display:none}');
  });
});
