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

  test('pinned session rows expose a drag handle and reorder callbacks', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="session-drag-handle" draggable={true}');
    expect(source).toContain('onDragStart={() => beginPinnedDrag(s.id)}');
    expect(source).toContain('onDrop={() => dropPinned(s.id)}');
    expect(source).toContain('reorderPinned={reorderPinned}');
    expect(styles).toContain('.session-drag-handle{display:inline-flex');
    expect(styles).toContain('.session-item.pinned{grid-template-columns:18px 22px minmax(0,1fr) 28px}');
    expect(styles).toContain('.session-item.drop-target{border-color:var(--accent)');
  });

  test('active rail buttons have per-mode classes and per-mode colors', () => {
    const source = app();
    for (const mode of ['chat', 'cron', 'memory', 'images', 'workspace', 'settings']) {
      expect(source).toContain(`nav-${mode}`);
      expect(css()).toContain(`.rail-btn.nav-${mode}.active`);
    }
  });
});
