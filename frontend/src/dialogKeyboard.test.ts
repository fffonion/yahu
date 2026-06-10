import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('dialog keyboard handling', () => {
  test('dialog Escape consumes the key event before image modal handlers', () => {
    const source = app();
    expect(source).toContain('event.stopImmediatePropagation();');
    expect(source).toContain("window.addEventListener('keydown', onKey, true)");
    expect(source).toContain("window.removeEventListener('keydown', onKey, true)");
    expect(source).toContain("if (event.defaultPrevented) return;");
  });
});
