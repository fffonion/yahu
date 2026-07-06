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
    expect(source).toContain('item.assistant_preview && <span className="user-minimap-assistant-preview">{item.assistant_preview}</span>');
    expect(source).toContain('formatNavigatorTime(item.timestamp)');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar');
    expect(styles).toContain('transition:width .18s ease,opacity .18s ease,background .18s ease');
    expect(styles).toContain('.user-minimap-popup{position:absolute;left:29px;');
    expect(styles).toContain('.user-minimap-assistant-preview{font-size:12px;line-height:1.35;color:color-mix(in srgb,var(--muted) 82%,transparent);');
  });

  test('renders every backend navigator item as a fixed-gap compact stack', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("className={`user-minimap-hit${activeIds.has(item.id) ? ' active' : ''}`}");
    expect(source).toContain('items.map((item) => <button type="button" className={`user-minimap-hit${activeIds.has(item.id) ? \' active\' : \'\'}`} key={item.id}');
    expect(source).toContain('data-nav-index={item.index} data-nav-total={item.total}');
    expect(source).not.toContain('const NAVIGATOR_RADIUS = 3;');
    expect(source).not.toContain('function navigatorVisibleItems');
    expect(source).not.toContain('function currentNavigatorIndex');
    expect(source).not.toContain('Math.min(1, item.position)');
    expect(source).not.toContain('function minimapHitStyle');
    expect(styles).toContain('.chat-user-minimap{position:absolute;left:10px;width:54px;z-index:90;display:flex;flex-direction:column;gap:0;top:50%;transform:translateY(-50%);height:80%;max-height:80%;overflow-x:hidden;overflow-y:auto;overscroll-behavior:contain;pointer-events:auto;mask-image:linear-gradient(to bottom,transparent 0%,black 6px,black calc(100% - 6px),transparent 100%);-webkit-mask-image:linear-gradient(to bottom,transparent 0%,black 6px,black calc(100% - 6px),transparent 100%);scrollbar-width:none}');
    expect(styles).toContain('.chat-user-minimap::-webkit-scrollbar{display:none;width:0;height:0}');
    expect(styles).not.toContain('max-height:80%;overflow-y:auto;pointer-events:auto;mask-image:linear-gradient');
    expect(styles).toContain('.user-minimap-hit{position:relative;width:54px;height:8px;border:0;background:transparent;padding:0;display:flex;align-items:center;justify-content:flex-start;pointer-events:auto;cursor:pointer;flex-shrink:0}');
    expect(styles).toContain('.user-minimap-bar{width:9px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar,.user-minimap-hit:focus-visible .user-minimap-bar{width:21px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit:hover) .user-minimap-bar{width:17px;');
  });

  test('uses backend full user-turn list and highlights the current visible range', () => {
    const source = app();
    expect(source).not.toContain('function navigatorBarTop(index: number, total: number)');
    expect(source).not.toContain('top: navigatorBarTop(visibleIndex, visibleCount),');
    expect(source).not.toContain('style={minimapHitStyle(visibleIndex, visibleItems.length)}');
    expect(source).toContain('const [activeNavigatorIds, setActiveNavigatorIds] = useState<Set<string>>(() => new Set());');
    expect(source).toContain('function activeNavigatorIdsForVisibleRange(scroller: HTMLElement | null, items: UserMessageNavItem[]): Set<string>');
    expect(source).toContain('if (itemNumeric <= end && nextNumeric > start) active.add(entry.item.id);');
    expect(source).toContain('activeIds={activeNavigatorIds}');
    expect(source).toContain('setUserMessageNav(Array.isArray(body.data) ? body.data : []);');
  });

  test('reserves chat history space for minimap before data loads and keeps minimap on mobile', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('return <main className="main-panel chat-main-panel">');
    expect(styles).toContain('.chat-main-panel .chat-scroll{padding-left:78px}');
    expect(styles).not.toContain('.main-panel:has(.chat-user-minimap) .chat-scroll{padding-left:78px}');
    expect(styles).toContain('@media (max-width:760px){.chat-main-panel .chat-scroll{padding-left:46px}.chat-user-minimap{display:flex;left:6px;top:50%;bottom:auto;transform:translateY(-50%);width:34px;height:80%;max-height:80%;overflow-x:hidden;overflow-y:auto;mask-image:none;');
    expect(styles).toContain('.user-minimap-hit{width:34px;flex-shrink:0}');
    expect(styles).not.toContain('.chat-user-minimap{display:none}');
  });
});
