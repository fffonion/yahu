import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('session right-click context menu', () => {
  test('session rows expose a custom context menu with rename and delete actions', () => {
    const source = app();
    expect(source).toContain('onContextMenu');
    expect(source).toContain('session-context-menu');
    expect(source).toContain('Rename session');
    expect(source).toContain('Delete session');
    expect(source).toContain('renameSession');
    expect(source).toContain('deleteSession');
  });

  test('rename and delete use Hermes session API endpoints', () => {
    const source = app();
    expect(source).toContain("method: 'PATCH'");
    expect(source).toContain('JSON.stringify({ title: nextTitle })');
    expect(source).toContain("method: 'DELETE'");
    expect(source).toContain('/api/sessions/${encodeURIComponent(session.id)}');
  });

  test('context menu is positioned above the chat sidebar and styled as a menu', () => {
    const styles = css();
    expect(styles).toContain('.session-context-menu{position:fixed');
    expect(styles).toContain('.session-context-menu button.danger');
  });
});
