import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('composer enter key behaviour', () => {
  test('defaults to Enter sending and persists the setting', () => {
    const source = app();
    expect(source).toContain("type ComposerEnterMode = 'enter-send' | 'enter-newline'");
    expect(source).toContain("const COMPOSER_ENTER_MODE_KEY = 'composerEnterMode'");
    expect(source).toContain("normalizeComposerEnterMode(localStorage.getItem(COMPOSER_ENTER_MODE_KEY))");
    expect(source).toContain('localStorage.setItem(COMPOSER_ENTER_MODE_KEY, composerEnterMode)');
    expect(source).toContain('composerEnterMode={composerEnterMode}');
    expect(source).toContain('setComposerEnterMode={setComposerEnterMode}');
  });

  test('textarea sends on plain Enter by default and sends on Ctrl/Cmd Enter in reverse mode', () => {
    const source = app();
    expect(source).toContain("if (e.key !== 'Enter' || e.shiftKey || (e.nativeEvent as KeyboardEvent).isComposing) return");
    expect(source).toContain('const modified = e.metaKey || e.ctrlKey');
    expect(source).toContain("const shouldSend = props.composerEnterMode === 'enter-newline' ? modified : !modified");
    expect(source).toContain('e.preventDefault(); props.sendMessage();');

  });

  test('settings exposes both enter key modes', () => {
    const source = app();
    const translations = i18n();
    expect(source).toContain('settings.composerEnterMode');
    expect(source).toContain('<option value="enter-send">{t(\'settings.enterSend\')}</option>');
    expect(source).toContain('<option value="enter-newline">{t(\'settings.enterNewline\')}</option>');
    expect(translations).toContain('settings.enterSend');
    expect(translations).toContain('Ctrl+Enter');
  });
});
