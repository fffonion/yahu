import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('settings API configuration display', () => {
  test('settings shows the actual Hermes API URL separately from the browser proxy base', () => {
    const source = app();
    const translations = i18n();
    expect(source).toContain("fetch('/runtime-config')");
    expect(source).toContain('const [apiServerUrl, setApiServerUrl] = useState');
    expect(source).toContain('apiServerUrl={apiServerUrl}');
    expect(source).toContain("<label><span>{t('settings.apiUrl')}</span><input value={props.apiServerUrl || '—'} readOnly /></label>");
    expect(source).toContain("<label><span>{t('settings.apiProxyBase')}</span><input value={props.apiBase}");
    expect(source).not.toContain("t('settings.apiBase')");
    expect(translations).toContain("'settings.apiUrl'");
    expect(translations).toContain("'settings.apiProxyBase'");
    expect(translations).not.toContain('Hermes API base');
  });
});
