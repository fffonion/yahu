import { describe, expect, test } from 'bun:test';
import { t, setLang } from './i18n';

describe('composer placeholder copy', () => {
  test('message composer placeholder is blank in every language', () => {
    for (const lang of ['en', 'zh-CN', 'zh-TW', 'ja'] as const) {
      setLang(lang);
      expect(t('chat.inputPlaceholder')).toBe('');
    }
  });
});
