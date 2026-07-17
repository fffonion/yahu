import { describe, expect, test } from 'bun:test';
import { getLang, setLang, t } from './i18n';

describe('composer placeholder copy', () => {
  test('message composer placeholder is blank in every language without changing the active language', () => {
    const initialLang = getLang();
    try {
      for (const lang of ['en', 'zh-CN', 'zh-TW', 'ja'] as const) {
        setLang(lang);
        expect(t('chat.inputPlaceholder')).toBe('');
      }
    } finally {
      setLang(initialLang);
    }
    expect(getLang()).toBe(initialLang);
  });
});
