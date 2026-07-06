import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat user message navigator', () => {
  test('loads user-message navigator from a separate lazy endpoint', () => {
    const source = app();
    expect(source).toContain("type UserMessageNavItem = { id: string; role: 'user'; content: string; assistant_preview?: string; timestamp?: string | number; position: number; index: number; total: number }");
    expect(source).toContain("const [userMessageNav, setUserMessageNav] = useState<UserMessageNavItem[]>([]);");
    expect(source).toContain("fetch(`/chat/user-nav/${encodeURIComponent(sessionId)}`)");
    expect(source).not.toContain('setUserMessageNav(visibleMessages');
    expect(source).toContain('setUserMessageNav(Array.isArray(body.data) ? body.data : []);');
    expect(source).toContain('updateSessionMessageCount(sessionId, body.total);');
  });

  test('clicking a navigator row loads an around-message page before scrolling', () => {
    const source = app();
    expect(source).toContain("const jumpToMessage = useCallback(async (sessionId: string, messageId: string) => {");
    expect(source).toContain("params.set('around', around);");
    expect(source).toContain('pendingJumpMessageIdRef.current = messageId;');
    expect(source).toContain("document.querySelector(`[data-message-id=\"${CSS.escape(targetId)}\"]`)");
    expect(source).toContain('onJumpToMessage={jumpToMessage}');
  });

  test('renders half-length bars with popup content final assistant preview and time', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function ChatUserNavigator(');
    expect(source).toContain('className="chat-user-minimap"');
    expect(source).toContain('className="user-minimap-popup"');
    expect(source).toContain('entry.item.assistant_preview && <span className="user-minimap-assistant-preview">{entry.item.assistant_preview}</span>');
    expect(source).toContain('formatNavigatorTime(entry.item.timestamp)');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar');
    expect(styles).toContain('transition:width .18s ease,opacity .18s ease,background .18s ease');
    expect(styles).toContain('.user-minimap-popup{position:absolute;left:calc(100% + 10px);');
    expect(styles).toContain('.user-minimap-assistant-preview{font-size:12px;line-height:1.35;color:color-mix(in srgb,var(--muted) 82%,transparent);');
  });

  test('shows at most three bars before and after the current bar without default selection width', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const NAVIGATOR_RADIUS = 3;');
    expect(source).toContain('const totalSlots = NAVIGATOR_RADIUS * 2 + 1; // always 7');
    expect(source).toContain('const start = Math.max(0, Math.min(centerIndex - half, items.length - totalSlots));');
    expect(source).toContain('const end = start + totalSlots;');
    expect(source).toContain('return items.slice(start, end).map((item, index) => ({ item, index: start + index, distance: start + index - centerIndex }));');
    expect(source).toContain('style={minimapHitStyle(visibleIndex, visibleItems.length)}');
    expect(source).not.toContain('function navigatorBarWidth');
    expect(source).not.toContain('--minimap-bar-width');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar,.user-minimap-hit:focus-visible .user-minimap-bar{width:21px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit:hover) .user-minimap-bar{width:17px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit+ .user-minimap-hit:hover) .user-minimap-bar{width:13px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit+.user-minimap-hit+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit+ .user-minimap-hit+ .user-minimap-hit:hover) .user-minimap-bar{width:9px;');
  });

  test('places visible minimap bars in a compact equal-spaced stack like the Codex reference', () => {
    const source = app();
    expect(source).toContain('function navigatorBarTop(index: number, total: number)');
    expect(source).toContain('const compactGapPx = Math.max(5, Math.min(12, 220 / (total - 1)));');
    expect(source).toContain('return `calc(50% + ${offsetPx.toFixed(1)}px)`;');
    expect(source).toContain('top: navigatorBarTop(visibleIndex, visibleCount),');
    expect(source).not.toContain('Math.max(0, Math.min(1, item.position)) * 100');
    expect(source).not.toContain('(index / (items.length - 1)) * 100');
  });

  test('reserves chat history space for minimap before data loads and keeps minimap on mobile', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('return <main className="main-panel chat-main-panel">');
    expect(styles).toContain('.chat-main-panel .chat-scroll{padding-left:78px}');
    expect(styles).not.toContain('.main-panel:has(.chat-user-minimap) .chat-scroll{padding-left:78px}');
    expect(styles).toContain('@media (max-width:760px){.chat-main-panel .chat-scroll{padding-left:46px}.chat-user-minimap{display:block;');
    expect(styles).toContain('.user-minimap-hit{width:34px}');
    expect(styles).not.toContain('.chat-user-minimap{display:none}');
  });
});
