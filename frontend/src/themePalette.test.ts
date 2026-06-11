import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('full palette theme control', () => {
  test('theme picker exposes full palettes and no separate light/dark action buttons', () => {
    const source = app();
    expect(source).toContain("type Theme = 'hermes-light' | 'hermes-dark' | 'vscode-light-plus' | 'vscode-dark-plus'");
    expect(source).toContain("{ id: 'vscode-dark-plus', label: 'VS Code Dark+' }");
    expect(source).toContain("{ id: 'vscode-light-plus', label: 'VS Code Light+' }");
    expect(source).toContain("<label><span>{t('theme.theme')}</span><select value={theme}");
    expect(source).not.toContain("onClick={() => setTheme('dark')}");
    expect(source).not.toContain("onClick={() => setTheme('light')}");
    expect(source).not.toContain('setSkin');
    expect(source).not.toContain('data-skin');
  });

  test('themes define complete UI and editor palettes copied from Hermes WebUI and VS Code', () => {
    const styles = css();
    expect(styles).toContain(':root[data-theme="vscode-dark-plus"]{--bg:#1e1e1e;--sidebar:#181818;--surface:#252526;--surface-2:#2d2d30;');
    expect(styles).toContain('--editor-bg:#1e1e1e;--editor-text:#d4d4d4;--syntax-keyword:#569cd6;--syntax-string:#ce9178;--syntax-number:#b5cea8;--syntax-comment:#6a9955;');
    expect(styles).toContain(':root[data-theme="vscode-light-plus"]{--bg:#ffffff;--sidebar:#f3f3f3;--surface:#ffffff;--surface-2:#f3f3f3;');
    expect(styles).toContain('--editor-bg:#ffffff;--editor-text:#333333;--syntax-keyword:#0000ff;--syntax-string:#a31515;--syntax-number:#098658;--syntax-comment:#008000;');
    expect(styles).toContain(':root[data-theme="hermes-dark"]{--bg:#0D0D1A;--sidebar:#141425;--surface:#1A1A2E;');
    expect(styles).toContain(':root[data-theme="hermes-light"]{--bg:#FEFCF7;--sidebar:#FAF7F0;--surface:#F3EEE3;');
    expect(styles).toContain(':root[data-theme="vscode-light-plus"] .send-btn,:root[data-theme="vscode-light-plus"] .save-memory,:root[data-theme="hermes-light"] .send-btn,:root[data-theme="hermes-light"] .save-memory{color:#fff}');
  });

  test('workspace editor and markdown code use theme editor tokens', () => {
    const styles = css();
    expect(styles).toContain('.workspace-editor{width:100%;height:100%;min-height:0;resize:none;padding:14px;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;background:var(--editor-bg);color:var(--editor-text);');
    expect(styles).toContain('.workspace-code-highlight{margin:0;min-height:0;overflow:auto;padding:14px;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;background:var(--editor-bg);');
    expect(styles).toContain('.tok-keyword{color:var(--syntax-keyword);font-weight:700}');
    expect(styles).toContain('.tok-string{color:var(--syntax-string)}');
    expect(styles).toContain('.msg-body code{background:var(--code-inline-bg);color:var(--code-text)}');
  });
});
