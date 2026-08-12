import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const themeColor = (styles: string, theme: string, variable: string) => {
  const block = styles.match(new RegExp(`:root\\[data-theme="${theme}"\\]\\{([^}]+)\\}`))?.[1] || '';
  return block.match(new RegExp(`${variable}:(#[0-9a-fA-F]{6})`))?.[1] || '';
};
const luminance = (color: string) => color.slice(1).match(/../g)!.map((part) => parseInt(part, 16) / 255)
  .map((channel) => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4)
  .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index], 0);
const contrast = (a: string, b: string) => {
  const [bright, dark] = [luminance(a), luminance(b)].sort((left, right) => right - left);
  return (bright + 0.05) / (dark + 0.05);
};

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

  test('adds layered GUI palettes with distinct overlays, controls, messages, status, editor, and chart colors', () => {
    const source = app();
    const styles = css();
    for (const [id, label] of [
      ['gruvbox-material', 'Gruvbox Material'],
      ['github-dark-dimmed', 'GitHub Dark Dimmed'],
      ['codex-light', 'Codex Light'],
      ['codex-dark', 'Codex Dark'],
      ['claude-code-light', 'Claude Code Light'],
      ['claude-code-dark', 'Claude Code Dark'],
    ]) {
      expect(source).toContain(`{ id: '${id}', label: '${label}' }`);
      expect(styles).toContain(`:root[data-theme="${id}"]{`);
    }
    expect(source).not.toContain("solarized-dark");
    expect(styles).not.toContain(':root[data-theme="solarized-dark"]');
    expect(source).not.toContain('tokyo-night');
    expect(source).not.toContain('rose-pine-moon');
    expect(styles).not.toContain(':root[data-theme="tokyo-night"]');
    expect(styles).not.toContain(':root[data-theme="rose-pine-moon"]');
    expect(styles).toContain('--panel-raised:var(--surface);--panel-overlay:var(--surface);--control-bg:var(--surface-2);--control-hover:var(--accent-soft);');
    expect(styles).toContain('--user-bubble:var(--accent-soft);--assistant-bubble:var(--surface);--tool-surface:var(--surface-2);');
    expect(styles).toContain('--info:var(--accent);--warning:var(--accent-2);--focus-ring:var(--accent-soft);');
    expect(styles).toContain('--chart-0:var(--accent);--chart-1:var(--accent-2);');
    expect(styles).toContain(':root[data-theme="gruvbox-material"]{--bg:#1d2021;--sidebar:#202324;--surface:#282828;--surface-2:#32302f;--panel-raised:#3c3836;--panel-overlay:#45403d;');
    expect(styles).toContain(':root[data-theme="github-dark-dimmed"]{--bg:#1c2128;--sidebar:#22272e;--surface:#2d333b;--surface-2:#373e47;--panel-raised:#444c56;--panel-overlay:#545d68;');
    expect(styles).toContain(':root[data-theme="codex-light"]{--bg:#ffffff;--sidebar:#f5f5f5;--surface:#ffffff;--surface-2:#f7f8fa;');
    expect(styles).toContain(':root[data-theme="codex-dark"]{--bg:#171717;--sidebar:#202020;--surface:#242424;--surface-2:#2b2b2b;');
    expect(styles).toContain(':root[data-theme="claude-code-light"]{--bg:#faf9f5;--sidebar:#f5f4ed;--surface:#ffffff;--surface-2:#f5f4ed;');
    expect(styles).toContain(':root[data-theme="claude-code-dark"]{--bg:#30302e;--sidebar:#262624;--surface:#30302e;--surface-2:#262624;');
    expect(styles).toContain(':root[data-theme="codex-light"]{--bg:#ffffff;--sidebar:#f5f5f5;--surface:#ffffff;--surface-2:#f7f8fa;--panel-raised:#ffffff;--panel-overlay:#ffffff;--text:#1a1c1f;--muted:#68727c;--border:#d9dde3;--border-strong:#b8c0ca;--accent:#339cff;');
    expect(styles).toContain('--accent:#d4d4d4;--accent-2:#0ea5e9;--accent-soft:rgba(212,212,212,.12);--danger:#f47067;--green:#6ccf88;--info:#339cff;--warning:#e2b93b;');
    expect(styles).toContain('--accent:#d97757;--accent-2:#cc785c;');
    expect(styles).toContain(':root[data-theme="codex-light"] .sidebar .rail-btn.active,:root[data-theme="codex-dark"] .sidebar .rail-btn.active');
    expect(styles).toContain(':root[data-theme="codex-light"] .send-btn,:root[data-theme="codex-dark"] .send-btn');
    expect(styles).toContain(':root[data-theme="codex-light"] .send-btn,:root[data-theme="codex-dark"] .send-btn,:root[data-theme="codex-light"] .composer-primary-btn.is-stop,:root[data-theme="codex-dark"] .composer-primary-btn.is-stop{background:var(--text);border-color:var(--text);color:var(--bg);box-shadow:none}');
    expect(styles).toContain(':root[data-theme="codex-light"] .reasoning-view-toggle.active,:root[data-theme="codex-dark"] .reasoning-view-toggle.active');
  });

  test('tool icon semantic colors are shared by every theme while the surrounding UI remains theme-specific', () => {
    const styles = css();
    expect(styles).toContain('--tool-terminal:#e2b93b;--tool-file:#0ea5e9;--tool-search:#339cff;--tool-browser:#60a5fa;--tool-media:#ec4899;');
    expect(styles).toContain('--tool-knowledge:#a78bfa;--tool-automation:#8b5cf6;--tool-planning:#e2b93b;--tool-home:#22c55e;--tool-communication:#ec4899;--tool-audio:#f59e0b;');
    for (const tone of ['terminal', 'file', 'search', 'browser', 'media', 'knowledge', 'automation', 'planning', 'home', 'communication', 'audio']) {
      expect(styles).toContain(`.tool-inline-icon.tool-icon-${tone}{color:var(--tool-${tone})}`);
    }
    expect(styles).not.toContain(':root[data-theme="codex-dark"] .tool-inline-icon.tool-icon-terminal');
    expect(styles).not.toContain(':root[data-theme="vscode-dark-plus"] .tool-inline-icon.tool-icon-terminal');
  });

  test('new theme secondary text keeps readable contrast against the page', () => {
    const styles = css();
    for (const theme of ['gruvbox-material', 'github-dark-dimmed', 'codex-light', 'codex-dark', 'claude-code-light', 'claude-code-dark']) {
      expect(contrast(themeColor(styles, theme, '--muted'), themeColor(styles, theme, '--bg'))).toBeGreaterThanOrEqual(4.5);
    }
  });

  test('semantic theme layers drive interactive UI instead of stopping at page foreground and background', () => {
    const styles = css();
    expect(styles).toContain('.chat-header,.image-toolbar{background:var(--header-bg)}');
    expect(styles).toContain('.session-item:hover,.file-row:hover,.skill-row:hover{background:var(--control-hover)}');
    expect(styles).toContain('.session-item.active,.skill-row.active{background:var(--selection-bg);border:1px solid var(--selection-border);box-shadow:none}');
    expect(styles).toContain('.msg-row.user .msg-content{background:var(--user-bubble)');
    expect(styles).toContain('.msg-row.assistant .msg-content,.msg-row.system .msg-content{background:var(--assistant-bubble)');
    expect(styles).toContain('.tool-card,.turn-detail-body{background:var(--tool-surface)}');
    expect(styles).toContain('.composer-wrap,.composer-box textarea{background:var(--composer-bg)}');
    expect(styles).toContain('.dialog-card,.session-context-menu,.skill-context-menu,.skill-file-context-menu,.workspace-context-menu,.image-context-menu{background:var(--panel-overlay)}');
    expect(styles).toContain('.insights-main{--insights-plot-left:4.1667%;--insights-plot-right:2.5%;');
    expect(styles).not.toContain('.insights-main{grid-column:2 / -1;--chart-0:#14b8a6;');
    expect(styles).toContain('.settings-content{grid-template-columns:minmax(0,920px);justify-content:start}');
    expect(styles).toContain('.settings-content label{background:transparent}');
    expect(styles).toContain('.settings-content input,.settings-content select{background-color:var(--control-bg);border-color:var(--border-strong)}');
    expect(styles).toContain('.settings-content select{cursor:pointer;padding-right:38px;background-image:linear-gradient(45deg,transparent 50%,var(--muted) 50%),linear-gradient(135deg,var(--muted) 50%,transparent 50%);');
    expect(styles).toContain('.rail-btn{width:42px;height:42px;border:1px solid transparent;');
    expect(styles).toContain('.sidebar .rail-btn.active{color:var(--rail-accent,var(--accent));border-color:var(--rail-accent,var(--accent));box-shadow:none}');
    expect(styles).not.toContain('.session-item.active,.skill-row.active{background:var(--selection-bg);box-shadow:inset 3px 0 0 var(--selection-border)}');
    expect(styles).not.toContain('.sidebar .rail-btn.active{color:var(--rail-accent,var(--accent));box-shadow:inset 3px 0 0 var(--rail-accent,var(--accent))}');
  });

  test('native select menus follow the active light or dark palette', () => {
    const styles = css();
    expect(styles).toContain(':root{color-scheme:light}:root.dark{color-scheme:dark}');
    expect(styles).toContain('select option{background:var(--control-bg);color:var(--text)}');
  });

  test('workspace editor and markdown code use theme editor tokens', () => {
    const styles = css();
    expect(styles).toContain('.workspace-editor{width:100%;height:100%;min-height:0;resize:none;padding:14px;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;background:var(--editor-bg);color:var(--editor-text);');
    expect(styles).toContain('.workspace-code-highlight{margin:0;min-height:0;overflow:auto;padding:14px;font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;background:var(--editor-bg);');
    expect(styles).toContain('.workspace-editor-textarea{min-height:0;width:100%;height:100%;resize:none;margin:0;padding:14px;');
    expect(styles).toContain('font:13px/1.55 ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;color:var(--editor-text);caret-color:var(--editor-text);background:var(--editor-bg);');
    expect(styles).toContain('.workspace-editor-highlight{display:none;');
    expect(styles).toContain('.tok-keyword{color:var(--syntax-keyword)}');
    expect(styles).not.toContain('.tok-keyword{color:var(--syntax-keyword);font-weight:700}');
    expect(styles).toContain('.tok-string{color:var(--syntax-string)}');
    expect(styles).toContain('.msg-body code{background:var(--code-inline-bg);color:var(--code-text)}');
  });
});
