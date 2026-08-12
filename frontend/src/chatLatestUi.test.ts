import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const transcript = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const translations = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('chat latest navigation UI', () => {
  test('renders a bottom-right latest control that loads latest history and sends an expansion token', () => {
    const source = app();
    expect(source).toContain('className="chat-latest-overlay chat-floating-controls"');
    expect(source).toContain('className="chat-latest-button"');
    expect(source).toContain('const latestButtonVisible = props.hasNewer || showLatestButton;');
    expect(source).toContain('{(latestButtonVisible || isSmallLandscape || isFullscreen) && <div className="chat-latest-overlay chat-floating-controls">');
    expect(source).toContain("props.loadMessageWindow(props.activeSessionId, 'latest')");
    expect(source).toContain('forceOpenLatestDetailToken={forceOpenLatestDetailToken}');
    expect(source).toContain("aria-label={t('chat.jumpLatest')}");
  });

  test('forces only the selected latest detail group open and lazy-loads it', () => {
    const source = transcript();
    expect(source).toContain('latestExpandableDetailGroupId(turnDetailItems, visibleMessages, false)');
    expect(source).not.toContain('latestExpandableDetailGroupId(turnDetailItems, visibleMessages, streaming)');
    expect(source).toContain('const detailForceOpenToken = forceOpenLatestDetailToken;');
    expect(source).toContain('forceOpenToken={item.id === forceOpenDetailGroupId ? detailForceOpenToken : 0}');
    expect(source).toContain('if (!forceOpenToken) return;');
    expect(source).toContain('setOpen(true);');
    expect(source).toContain('loadDetails();');
  });

  test('positions a square accessible control over the lower-right chat viewport on desktop and mobile', () => {
    const css = styles();
    expect(css).toContain('.chat-latest-overlay{grid-row:2;grid-column:1;align-self:end;justify-self:end;');
    expect(css).toContain('.chat-floating-controls{display:flex;flex-direction:column;align-items:flex-end;gap:8px}');
    expect(css).toContain('.small-landscape-fullscreen-button{width:40px;height:40px;');
    expect(css).toContain('.chat-latest-button{width:40px;height:40px;');
    expect(css).toContain('@media(max-width:760px){.chat-latest-overlay{padding:10px}');
    expect(translations()).toContain("'chat.jumpLatest': { en: 'Jump to latest'");
    expect(translations()).toContain("'chat.enterFullscreen': { en: 'Enter fullscreen'");
    expect(translations()).toContain("'chat.exitFullscreen': { en: 'Exit fullscreen'");
  });
});
