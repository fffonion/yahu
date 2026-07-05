import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat user message navigator', () => {
  test('loads user-message navigator from a separate lazy endpoint', () => {
    const source = app();
    expect(source).toContain("type UserMessageNavItem = { id: string; role: 'user'; content: string; timestamp?: string | number; position: number; index: number; total: number }");
    expect(source).toContain("const [userMessageNav, setUserMessageNav] = useState<UserMessageNavItem[]>([]);");
    expect(source).toContain("fetch(`/chat/user-nav/${encodeURIComponent(sessionId)}`)");
    expect(source).not.toContain('setUserMessageNav(visibleMessages');
  });

  test('clicking a navigator row loads an around-message page before scrolling', () => {
    const source = app();
    expect(source).toContain("const jumpToMessage = useCallback(async (sessionId: string, messageId: string) => {");
    expect(source).toContain("params.set('around', around);");
    expect(source).toContain('pendingJumpMessageIdRef.current = messageId;');
    expect(source).toContain("document.querySelector(`[data-message-id=\"${CSS.escape(targetId)}\"]`)");
    expect(source).toContain('onJumpToMessage={jumpToMessage}');
  });

  test('renders half-length bars with popup content and time', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function ChatUserNavigator(');
    expect(source).toContain('className="chat-user-minimap"');
    expect(source).toContain('className="user-minimap-popup"');
    expect(source).toContain('formatNavigatorTime(entry.item.timestamp)');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar');
    expect(styles).toContain('transition:width .18s ease,opacity .18s ease,background .18s ease');
    expect(styles).toContain('.user-minimap-popup{position:absolute;left:calc(100% + 10px);');
  });

  test('shows at most three bars before and after the current bar without default selection width', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const NAVIGATOR_RADIUS = 3;');
    expect(source).toContain('const start = Math.max(0, centerIndex - NAVIGATOR_RADIUS);');
    expect(source).toContain('const end = Math.min(items.length, centerIndex + NAVIGATOR_RADIUS + 1);');
    expect(source).toContain('return items.slice(start, end).map((item, index) => ({ item, index: start + index, distance: start + index - centerIndex }));');
    expect(source).toContain('style={minimapHitStyle(visibleIndex, visibleItems.length)}');
    expect(source).not.toContain('function navigatorBarWidth');
    expect(source).not.toContain('--minimap-bar-width');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar,.user-minimap-hit:focus-visible .user-minimap-bar{width:21px;');
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

  test('reserves desktop history space for minimap but releases it on mobile', () => {
    const styles = css();
    expect(styles).toContain('.main-panel:has(.chat-user-minimap) .chat-scroll{padding-left:78px}');
    expect(styles).toContain('@media (max-width:760px){.chat-user-minimap{display:none}.main-panel:has(.chat-user-minimap) .chat-scroll{padding-left:10px}}');
  });
});
