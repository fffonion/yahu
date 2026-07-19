import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('shared code wrapping setting', () => {
  test('defaults on, persists, and drives one root layout class', () => {
    const source = app();
    expect(source).toContain("const CODE_WRAP_KEY = 'codeWrap'");
    expect(source).toContain("function readCodeWrap() { return localStorage.getItem(CODE_WRAP_KEY) !== '0'; }");
    expect(source).toContain('const [codeWrap, setCodeWrap] = useState(readCodeWrap);');
    expect(source).toContain("localStorage.setItem(CODE_WRAP_KEY, codeWrap ? '1' : '0')");
    expect(source).toContain("${codeWrap ? 'code-wrap' : 'code-nowrap'}");
  });

  test('settings exposes the shared wrapping choice', () => {
    const source = app();
    const translations = i18n();
    expect(source).toContain('codeWrap={codeWrap}');
    expect(source).toContain('setCodeWrap={setCodeWrap}');
    expect(source).toContain("<span>{t('settings.codeWrap')}</span>");
    expect(source).toContain("value={props.codeWrap ? 'on' : 'off'}");
    expect(source).toContain("props.setCodeWrap(e.target.value === 'on')");
    expect(translations).toContain("'settings.codeWrap'");
    expect(translations).toContain("'settings.codeWrapOn'");
    expect(translations).toContain("'settings.codeWrapOff'");
  });

  test('one root class wraps workspace editors, previews, chat code, and file tool results', () => {
    const styles = css();
    expect(styles).toContain('.app-shell.code-wrap .workspace-code-highlight,.app-shell.code-wrap .workspace-editor,.app-shell.code-wrap .workspace-editor-textarea');
    expect(styles).toContain('.app-shell.code-wrap .msg-body pre,.app-shell.code-wrap .msg-body pre code');
    expect(styles).toContain('.app-shell.code-wrap .tool-code-source{white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word}');
    expect(styles).toContain('.app-shell.code-wrap .tool-code-block code{min-width:0}');
    expect(styles).toContain('.app-shell.code-wrap .tool-code-line{grid-template-columns:5.5ch minmax(0,1fr)}');
    expect(styles).toContain('.app-shell.code-wrap .tool-code-diff .tool-code-line{grid-template-columns:2ch minmax(0,1fr)}');
    expect(styles).toContain('.app-shell.code-nowrap .workspace-code-highlight,.app-shell.code-nowrap .workspace-editor,.app-shell.code-nowrap .workspace-editor-textarea');
    expect(styles).toContain('.app-shell.code-nowrap .tool-code-source{white-space:pre;overflow-wrap:normal;word-break:normal}');
  });
});
