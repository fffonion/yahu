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

  test('confirm dialogs resolve from mouse clicks instead of relying on form submit', () => {
    const source = app();
    expect(source).toContain("<button type=\"button\" className={dialog.danger ? 'danger' : ''} onClick={() => finish(dialog.variant === 'prompt' ? value : true)}>{dialog.variant === 'prompt' ? t('dialog.save') : t('dialog.confirm')}</button>");
    expect(source).toContain('onSubmit={(event) => { event.preventDefault(); }}');
  });

});
