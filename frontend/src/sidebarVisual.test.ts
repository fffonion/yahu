import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('sidebar visual behavior', () => {
  test('unpinned session pin buttons are hidden until the row is hovered or focused', () => {
    const styles = css();
    expect(app()).toContain("pinned ? 'pinned' : ''");
    expect(styles).toContain('.pin-hit{opacity:0;pointer-events:none');
    expect(styles).toContain('.session-item:hover .pin-hit,.session-item:focus-within .pin-hit,.session-item.pinned .pin-hit{opacity:1;pointer-events:auto');
  });

  test('active rail buttons have per-mode classes and per-mode colors', () => {
    const source = app();
    for (const mode of ['chat', 'cron', 'memory', 'images', 'workspace', 'settings']) {
      expect(source).toContain(`nav-${mode}`);
      expect(css()).toContain(`.rail-btn.nav-${mode}.active`);
    }
  });
});
