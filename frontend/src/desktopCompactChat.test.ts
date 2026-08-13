import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => [readFileSync(new URL('./App.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./chatMessage.ts', import.meta.url), 'utf8')].join('\n');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('desktop compact chat toggle', () => {
  test('composer renders details control instead of separate compact toggle ordering', () => {
    const source = app();
    expect(source).toContain("desktopCompactMessages={desktopCompactMessages}");
    expect(source).toContain("setDesktopCompactMessages={setDesktopCompactMessages}");
    expect(source).toContain('function ComposerDetailsControl');
    expect(source).toContain('onToggleReasoning={toggleReasoningVisibility}');
    expect(source).toContain('onToggleToolCalls={toggleToolCallVisibility}');
    expect(source).toContain('onToggleCompact={() => props.setDesktopCompactMessages(!props.desktopCompactMessages)}');
    expect(source).toContain('className="composer-details-option"');
  });

  test('desktop compact toggle uses localized compact mode copy', () => {
    const source = app();
    const messages = i18n();
    expect(source).toContain("t('chat.collapseDetails')");
    expect(messages).toContain("'chat.compactMode': { en: 'Compact mode', 'zh-CN': '紧凑模式', 'zh-TW': '緊湊模式', ja: 'コンパクトモード' }");
  });

  test('mobile collapsed composer keeps its footer inside the composer box', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap.composer-compact .composer-box{min-height:64px;overflow:hidden}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer{position:absolute;right:4px;bottom:4px;width:auto;height:38px;');
  });

  test('mobile collapsed composer has equal outer spacing and a centered send control', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap.composer-compact{padding:8px 10px calc(var(--mobile-bottom-nav-height) + 10px + env(safe-area-inset-bottom,0px))}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-box{width:calc(100% - 12px);min-height:46px;margin:0 6px;border-radius:18px}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer{right:5px;bottom:5px;width:36px;height:36px;padding:0;display:flex;align-items:center;justify-content:center;overflow:visible}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .composer-primary-btn{width:34px;height:34px;flex:0 0 34px}');
  });

  test('desktop compact mode uses flat left-aligned user turns separated without an outer card', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("chat-main-panel ${props.desktopCompactMessages ? 'desktop-compact-chat' : ''}${isMobile ? ' mobile-compact-chat' : ''}");
    expect(source).toContain('className={`desktop-turn-block${sessionStateOnly');
    expect(source).toContain('buildDesktopTurnBlocks(turnDetailItems)');
    expect(styles).toContain('.desktop-compact-chat .msg-content{background:transparent;border:0;border-radius:0;box-shadow:none;padding:0}');
    expect(styles).toContain('.desktop-compact-chat .tool-card,.mobile-compact-chat .tool-card{background:transparent;border:0;border-radius:0;box-shadow:none;overflow:visible}');
    expect(styles).toContain('.desktop-compact-chat .desktop-turn-block{border:0;border-bottom:1px solid var(--border);border-radius:0;background:transparent;box-shadow:none;padding:12px 0;');
    expect(styles).toContain('.desktop-compact-chat .desktop-turn-block:last-child{border-bottom:0}');
    expect(styles).toContain('.desktop-compact-chat .turn-detail-group,.mobile-compact-chat .turn-detail-group{max-width:100%;border:1px solid');
    expect(styles).toContain('.desktop-compact-chat .msg-row.user{margin-left:0;border-bottom:1px solid var(--border);padding-bottom:8px;margin-bottom:2px}');
    expect(styles).toContain('.desktop-compact-chat .msg-row.user .msg-content{grid-column:1;justify-self:start;max-width:100%;background:transparent}');
    expect(styles).toContain('@media(max-width:760px){.desktop-compact-view-toggle{display:none!important}');
  });

  test('chat message text containers do not paint flush inner bubbles', () => {
    const styles = css();
    const themeBubble = styles.lastIndexOf('.msg-row.user .msg-content{background:var(--user-bubble)');
    const flatBubble = styles.lastIndexOf('.chat-main-panel .msg-row.user .msg-content,.chat-main-panel .msg-row.assistant .msg-content,.chat-main-panel .msg-row.system .msg-content{background:transparent;border-color:transparent}');
    expect(themeBubble).toBeGreaterThan(-1);
    expect(flatBubble).toBeGreaterThan(themeBubble);
  });

  test('desktop chat fills the history width with equal minimap and right gutters without changing mobile', () => {
    const styles = css();
    expect(styles).toContain('@media(min-width:761px){.chat-main-panel .chat-scroll{padding-left:78px;padding-right:78px}');
    expect(styles).toContain('.chat-main-panel .msg-row,.chat-main-panel .turn-detail-group,.chat-main-panel .special-context-block,.chat-main-panel .session-state-message,.chat-main-panel .history-coverage-gap{max-width:none}');
    expect(styles).toContain('@media (max-width:760px){.chat-main-panel .chat-scroll{padding-left:44px}');

  });
});
