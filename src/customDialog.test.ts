import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('custom themed dialogs', () => {
  test('browser native prompt and confirm are not used', () => {
    const source = app();
    expect(source).not.toContain('window.prompt');
    expect(source).not.toContain('window.confirm');
    expect(source).not.toContain('window.alert');
    expect(source).toContain('CustomDialog');
    expect(source).toContain('requestPrompt');
    expect(source).toContain('requestConfirm');
  });

  test('dialog component uses app theme classes', () => {
    const styles = css();
    expect(styles).toContain('.dialog-backdrop');
    expect(styles).toContain('.dialog-card');
    expect(styles).toContain('.dialog-actions');
  });

  test('memory and settings sidebars do not show login-key protection copy', () => {
    const source = app();
    expect(source).not.toContain('Protected by WebUI login key.');
    expect(source).not.toContain('admin-note');
  });
});
