import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('desktop compact chat toggle', () => {
  test('composer renders compact toggle immediately after tool toggle', () => {
    const source = app();
    expect(source).toContain("desktopCompactMessages={desktopCompactMessages}");
    expect(source).toContain("setDesktopCompactMessages={setDesktopCompactMessages}");
    expect(source).toContain("reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`}");
    expect(source).toContain("tool-call-view-toggle ${props.showToolCalls ? 'active' : ''}`}");
    expect(source).toContain("desktop-compact-view-toggle ${props.desktopCompactMessages ? 'active' : ''}`}");
    expect(source.indexOf('reasoning-view-toggle')).toBeLessThan(source.indexOf('tool-call-view-toggle'));
    expect(source.indexOf('tool-call-view-toggle')).toBeLessThan(source.indexOf('desktop-compact-view-toggle'));
  });

  test('desktop compact toggle uses localized compact mode copy', () => {
    const source = app();
    const messages = i18n();
    expect(source).toContain("aria-label={t('chat.compactMode')}");
    expect(source).toContain("title={t('chat.compactMode')}");
    expect(source).not.toContain('Use card chat layout');
    expect(source).not.toContain('Use compact chat layout');
    expect(messages).toContain("'chat.compactMode': { en: 'Compact mode', 'zh-CN': '紧凑模式', 'zh-TW': '緊湊模式', ja: 'コンパクトモード' }");
  });

  test('desktop compact mode uses flat left-aligned user turns with a divider', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("chat-main-panel ${props.desktopCompactMessages ? 'desktop-compact-chat' : ''}${isMobile ? ' mobile-compact-chat' : ''}");
    expect(source).toContain('className="desktop-turn-block"');
    expect(source).toContain('buildDesktopTurnBlocks(turnDetailItems)');
    expect(styles).toContain('.desktop-compact-chat .msg-content{background:transparent;border:0;border-radius:0;box-shadow:none;padding:0}');
    expect(styles).toContain('.desktop-compact-chat .tool-card,.mobile-compact-chat .tool-card{background:transparent;border:0;border-radius:0;box-shadow:none;overflow:visible}');
    expect(styles).toContain('.desktop-compact-chat .desktop-turn-block{border:1px solid var(--border);border-radius:var(--radius-lg);');
    expect(styles).toContain('.desktop-compact-chat .msg-row.user{margin-left:0;border-bottom:1px solid var(--border);padding-bottom:8px;margin-bottom:2px}');
    expect(styles).toContain('.desktop-compact-chat .msg-row.user .msg-content{grid-column:1;justify-self:start;max-width:100%;background:transparent}');
    expect(styles).toContain('@media(max-width:760px){.desktop-compact-view-toggle{display:none!important}');
  });
});
