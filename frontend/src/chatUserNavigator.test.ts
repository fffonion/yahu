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

  test('renders shorter default bars with popup content final assistant preview and time', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function ChatUserNavigator(');
    expect(source).toContain('className="chat-user-minimap"');
    expect(source).toContain('className="user-minimap-popup"');
    expect(source).toContain('const [popup, setPopup] = useState<{ item: UserMessageNavItem; top: number } | null>(null);');
    expect(source).toContain('popup.item.assistant_preview && <span className="user-minimap-assistant-preview">{popup.item.assistant_preview}</span>');
    expect(source).toContain('formatNavigatorTime(popup.item.timestamp)');
    expect(styles).toContain('.user-minimap-bar{width:6px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar');
    expect(styles).toContain('transition:width .18s ease,opacity .18s ease,background .18s ease');
    expect(styles).toContain('.user-minimap-popup{position:absolute;left:29px;');
    expect(styles).toContain('display:grid;gap:7px;text-align:left;opacity:1;pointer-events:none;z-index:1;');
    expect(styles).not.toContain('.user-minimap-hit:hover .user-minimap-popup,.user-minimap-hit:focus-visible .user-minimap-popup{display:grid;opacity:1;transform:translateY(-50%) translateX(0)}');
    expect(styles).toContain('.user-minimap-assistant-preview{font-size:12px;line-height:1.35;color:color-mix(in srgb,var(--muted) 82%,transparent);');
  });

  test('renders every backend navigator item as a fixed-gap compact stack', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("className={`user-minimap-hit${activeIds.has(item.id) ? ' active' : ''}`}");
    expect(source).toContain('items.map((item) => <button type="button" className={`user-minimap-hit${activeIds.has(item.id) ? \' active\' : \'\'}`} key={item.id}');
    expect(source).toContain('className={`user-minimap-track${scrollFade.before ? \' can-scroll-before\' : \'\'}${scrollFade.after ? \' can-scroll-after\' : \'\'}`}');
    expect(source).toContain('onPointerEnter={(event) => { if (!isMobileNavigator) showPopup(item, event.currentTarget); }}');
    expect(source).toContain('data-nav-index={item.index} data-nav-total={item.total}');
    expect(source).not.toContain('const NAVIGATOR_RADIUS = 3;');
    expect(source).not.toContain('function navigatorVisibleItems');
    expect(source).not.toContain('function currentNavigatorIndex');
    expect(source).not.toContain('Math.min(1, item.position)');
    expect(source).not.toContain('function minimapHitStyle');
    expect(styles).toContain('.chat-user-minimap{position:absolute;left:10px;width:54px;z-index:90;top:50%;transform:translateY(-50%);max-height:var(--user-minimap-max-height,75%);overflow:visible;pointer-events:auto}');
    expect(styles).toContain('.user-minimap-track{display:flex;flex-direction:column;gap:0;max-height:var(--user-minimap-max-height,75%);overflow-x:hidden;overflow-y:auto;overscroll-behavior:contain;pointer-events:auto;touch-action:pan-y;-webkit-overflow-scrolling:touch;mask-image:none;-webkit-mask-image:none;scrollbar-width:none}');
    expect(styles).toContain('.user-minimap-track.can-scroll-before{mask-image:linear-gradient(to bottom,transparent 0%,black 18px,black 100%);-webkit-mask-image:linear-gradient(to bottom,transparent 0%,black 18px,black 100%)}');
    expect(styles).toContain('.user-minimap-track.can-scroll-after{mask-image:linear-gradient(to bottom,black 0%,black calc(100% - 18px),transparent 100%);-webkit-mask-image:linear-gradient(to bottom,black 0%,black calc(100% - 18px),transparent 100%)}');
    expect(styles).toContain('.user-minimap-track.can-scroll-before.can-scroll-after{mask-image:linear-gradient(to bottom,transparent 0%,black 18px,black calc(100% - 18px),transparent 100%);-webkit-mask-image:linear-gradient(to bottom,transparent 0%,black 18px,black calc(100% - 18px),transparent 100%)}');
    expect(styles).toContain('.user-minimap-track::-webkit-scrollbar{display:none;width:0;height:0}');
    expect(styles).not.toContain('height:80%;max-height:80%;');
    expect(styles).not.toContain('max-height:min(52dvh,420px)');
    expect(styles).toContain('.user-minimap-hit{position:relative;width:54px;height:8px;border:0;background:transparent;padding:0;display:flex;align-items:center;justify-content:flex-start;pointer-events:auto;cursor:pointer;flex-shrink:0}');
    expect(styles).toContain('.user-minimap-bar{width:6px;');
    expect(styles).toContain('.user-minimap-hit:hover .user-minimap-bar,.user-minimap-hit:focus-visible .user-minimap-bar{width:21px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit:hover) .user-minimap-bar{width:17px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit+ .user-minimap-hit:hover) .user-minimap-bar{width:13px;');
    expect(styles).toContain('.user-minimap-hit:hover+.user-minimap-hit+.user-minimap-hit+.user-minimap-hit .user-minimap-bar,.user-minimap-hit:has(+ .user-minimap-hit+ .user-minimap-hit+ .user-minimap-hit:hover) .user-minimap-bar{width:9px;');
    expect(styles.indexOf('.chat-user-minimap:has(.user-minimap-hit:hover) .user-minimap-hit:not(:hover) .user-minimap-bar{opacity:.42}')).toBeLessThan(styles.indexOf('.user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar'));
    expect(styles).toContain('.chat-user-minimap:has(.user-minimap-hit:hover) .user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar,.chat-user-minimap:has(.user-minimap-hit:hover) .user-minimap-hit:has(+ .user-minimap-hit:hover) .user-minimap-bar{width:17px;opacity:.9}');
    expect(styles.indexOf('.chat-user-minimap:has(.user-minimap-hit:hover) .user-minimap-hit:not(:hover) .user-minimap-bar{opacity:.42}')).toBeLessThan(styles.indexOf('.chat-user-minimap:has(.user-minimap-hit:hover) .user-minimap-hit:hover+.user-minimap-hit .user-minimap-bar'));
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
    expect(source).toContain('return <main className={`main-panel chat-main-panel ${props.desktopCompactMessages ? \'desktop-compact-chat\' : \'\'}${isMobile ? \' mobile-compact-chat\' : \'\'}`}>');
    expect(styles).toContain('.chat-main-panel .chat-scroll{grid-row:2;grid-column:1;padding-left:78px}');
    expect(styles).not.toContain('.main-panel:has(.chat-user-minimap) .chat-scroll{padding-left:78px}');
    expect(styles).toContain('@media (max-width:760px){.chat-main-panel .chat-scroll{padding-left:44px}.chat-user-minimap{left:6px;top:50%;bottom:auto;transform:translateY(-50%);width:34px;max-height:var(--user-minimap-max-height,75%);overflow:visible;pointer-events:auto}.user-minimap-track{width:34px;max-height:var(--user-minimap-max-height,75%)}');
    const mobileRule = styles.match(/@media \(max-width:760px\)\{\.chat-main-panel \.chat-scroll\{padding-left:(\d+)px\}\.chat-user-minimap\{left:(\d+)px;[^}]*width:(\d+)px;/);
    expect(mobileRule).not.toBeNull();
    const [, padding, left, width] = mobileRule || [];
    expect(Number(padding) - (Number(left) + Number(width))).toBe(4);
    expect(styles).toContain('.user-minimap-hit{width:34px;flex-shrink:0}');
    expect(styles).not.toContain('.chat-user-minimap{display:none}');
  });

  test('mobile minimap taps open a temporary popup and outside chat taps close it', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("const isMobileNavigator = useMediaQuery('(max-width: 760px)');");
    expect(source).toContain('const popupTimerRef = useRef<number | null>(null);');
    expect(source).toContain('window.setTimeout(() => setPopup(null), 3000);');
    expect(source).toContain("document.addEventListener('pointerdown', closeMobilePopupOnOutsidePointer);");
    expect(source).toContain('if (target && navRef.current?.contains(target)) return;');
    expect(source).toContain('if (isMobileNavigator) {');
    expect(source).toContain('event.preventDefault();');
    expect(source).toContain('event.stopPropagation();');
    expect(source).toContain('showPopup(item, event.currentTarget, true);');
    expect(source).toContain('onClick={(event) => handleNavigatorClick(item, event)}');
    expect(source).toContain('onPointerEnter={(event) => { if (!isMobileNavigator) showPopup(item, event.currentTarget); }}');
    expect(styles).toContain('@media (max-width:760px){.user-minimap-popup{left:28px;width:min(300px,calc(100vw - 48px));');
    expect(styles).not.toContain('.user-minimap-popup{display:none}}');
  });

  test('mobile minimap popup can be tapped again to jump to its user message', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const handlePopupClick = useCallback((item: UserMessageNavItem) => {');
    expect(source).toContain('hidePopup();\n    onJumpToMessage(sessionId, item.id);');
    expect(source).toContain('onClick={() => handlePopupClick(popup.item)}');
    expect(source).toContain('<button type="button" className="user-minimap-popup"');
    expect(styles).toContain('@media (max-width:760px){.user-minimap-popup{left:28px;width:min(300px,calc(100vw - 48px));max-width:calc(100vw - 48px);padding:9px 10px;border-radius:12px;z-index:3;pointer-events:auto}');
  });

  test('mobile minimap initially scrolls overflowing track to the active viewport bar', () => {
    const source = app();
    expect(source).toContain("const initialMobileScrollKeyRef = useRef('');");
    expect(source).toContain("initialMobileScrollKeyRef.current = '';");
    expect(source).toContain('if (!isMobileNavigator || !activeIds.size) return;');
    expect(source).toContain("const target = track.querySelector('.user-minimap-hit.active') as HTMLElement | null;");
    expect(source).toContain('track.scrollTop = Math.max(0, Math.min(track.scrollHeight - track.clientHeight, target.offsetTop - (track.clientHeight - target.clientHeight) / 2));');
  });

  test('sizes minimap from the chat history viewport and fades only scrollable edges', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('chatScrollRef={props.chatScrollRef}');
    expect(source).toContain('chatScrollRef: React.RefObject<HTMLElement | null>');
    expect(source).toContain('nav.style.setProperty(\'--user-minimap-max-height\', `${Math.floor(scroller.clientHeight * 0.75)}px`);');
    expect(source).toContain('className={`user-minimap-track${scrollFade.before ? \' can-scroll-before\' : \'\'}${scrollFade.after ? \' can-scroll-after\' : \'\'}`}');
    expect(source).toContain('before: track.scrollTop > 1');
    expect(source).toContain('after: track.scrollTop + track.clientHeight < track.scrollHeight - 1');
    expect(source).toContain("typeof ResizeObserver !== 'undefined' ? new ResizeObserver(updateNavigatorMetrics) : null;");
    expect(styles).toContain('max-height:var(--user-minimap-max-height,75%)');
    expect(styles).toContain('.user-minimap-track.can-scroll-before');
    expect(styles).toContain('.user-minimap-track.can-scroll-after');
    expect(styles).not.toContain('max-height:min(52dvh,420px)');
  });
});
