import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('composer details popover', () => {
  test('uses one settings trigger with a slider root and nested text menus', () => {
    const styles = css();
    const source = app();
    expect(source).toContain('composer-details-trigger');
    expect(source).toContain('composer-trigger-model');
    expect(source).toContain('composer-trigger-effort');

    expect(source).toContain('composer-details-page');
    expect(source).toContain('composer-submenu-row');
    expect(source).toContain('composer-model-list');
    expect(styles).toContain('.composer-details-popover{position:absolute;right:0;bottom:calc(100% + 10px);');
    expect(styles).toContain('.composer-details-advanced{display:flex;');
    expect(styles).toContain('.composer-submenu-row{');

  });
});
