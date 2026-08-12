import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('sidebar session section collapse', () => {
  test('pinned and recent headings toggle their own session lists', () => {
    const source = app();
    expect(source).toContain("useState<{ pinned: boolean; recent: boolean }>({ pinned: false, recent: false })");
    expect(source).toContain('aria-expanded={!collapsedSections.pinned}');
    expect(source).toContain('aria-expanded={!collapsedSections.recent}');
    expect(source).toContain('!collapsedSections.pinned && props.pinnedSessions.map');
    expect(source).toContain('!collapsedSections.recent && props.normalSessions.map');
    expect(source).toContain('{collapsedSections.pinned ? <ChevronRight /> : <ChevronDown />}');
    expect(source).toContain('{collapsedSections.recent ? <ChevronRight /> : <ChevronDown />}');
  });

  test('section headings keep button semantics without browser button chrome', () => {
    const styles = css();
    expect(styles).toContain('.section-label[type="button"]{width:100%;border:0;padding:0;background:transparent;text-align:left;font:inherit;color:inherit}');
    expect(styles).toContain('.section-label[type="button"]:focus-visible');
  });
});
