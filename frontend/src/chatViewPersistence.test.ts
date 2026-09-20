import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { migrateChatViewState } from './chatViewState';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('chat view persistence', () => {
  test('migrates legacy session view state to the canonical id and keeps the legacy position', () => {
    expect(migrateChatViewState({
      lastSessionId: 'legacy',
      positions: { legacy: 123, canonical: 456, other: 9 },
      anchors: { legacy: { id: 'message-1', topOffset: 12 }, canonical: { id: 'message-2', topOffset: 30 } },
    }, 'legacy', 'canonical')).toEqual({
      lastSessionId: 'canonical',
      positions: { canonical: 123, other: 9 },
      anchors: { canonical: { id: 'message-1', topOffset: 12 } },
    });
  });

  test('stores the last session and per-session scroll positions and anchors in localStorage', () => {
    const source = app();
    expect(source).toContain("const CHAT_VIEW_STATE_KEY = 'yahu.chat.view.v1';");
    expect(source).toContain('const readChatViewState = (): { lastSessionId: string; positions: Record<string, number>; anchors: Record<string, StoredChatAnchor> }');
    expect(source).toContain('state.positions[sessionId] = Math.max(0, Number(scrollTop));');
    expect(source).toContain('state.anchors[sessionId] = { id: anchor.id, topOffset: Number.isFinite(anchor.topOffset) ? anchor.topOffset : 0 };');
    expect(source).toContain('writeChatViewState(props.activeSessionId, el.scrollTop, anchor ? { id: anchor.id, topOffset: anchor.topOffset } : null);');
    expect(source).toContain("const followMode = followState.sessionId === watchedSessionId ? followState.mode : 'auto';");
    expect(source).toContain("const wasNearBottom = followMode === 'follow' || (followMode === 'auto' && !!chatScrollRef.current && isNearBottom(chatScrollRef.current));");
    expect(source).not.toContain('const viewportMatchesSaved = !Number.isFinite(savedScrollTop)');
  });

  test('uses the stored session when the chat route has no explicit session', () => {
    const source = app();
    expect(source).toContain("initialRoute.sessionId || initialChatView.lastSessionId || ''");
    expect(source).toContain('const initialChatView = readChatViewState();');
    expect(source).toContain('clearLastChatViewSession();');
  });

  test('restores the saved anchor before falling back to the saved scroll position', () => {
    const source = app();
    expect(source).toContain('const pendingHistoryScrollAnchorRef = useRef<{ sessionId: string; anchor: MessageScrollAnchor } | null>(null);');
    expect(source).toContain("pendingHistoryScrollAnchorRef.current = anchor ? { sessionId, anchor } : null;");
    expect(source).toContain('pendingHistoryScroll?.sessionId === activeSessionId ? pendingHistoryScroll.anchor : readChatViewAnchor(activeSessionId)');
    expect(source).toContain('const savedAnchor = readChatViewAnchor(activeSessionId);');
    expect(source).toContain('? { sessionId: activeSessionId, anchor: { id: savedAnchor.id, topOffset: savedAnchor.topOffset } }');
    expect(source).toContain("scrollLatestAfterRenderRef.current = { sessionId: activeSessionId, mode: 'restore' };");
    expect(source).toContain("if (pendingAnchor && restoreMessageScrollAnchor(scroller, pendingAnchor)) {");
    expect(source).toContain('pendingHistoryScrollAnchorRef.current = null;');
    expect(source).toContain('scroller.scrollTop = Math.min(Math.max(0, Number(savedTop)), Math.max(0, scroller.scrollHeight - scroller.clientHeight));');
    expect(source).toContain('window.setTimeout(restorePosition, 300);');
    expect(source).toContain('scroller.scrollTop = scroller.scrollHeight;');
  });

  test('marks live updates and explicit latest jumps as follow actions', () => {
    const source = app();
    expect(source).toContain("scrollLatestAfterRenderRef.current = { sessionId, mode: 'follow' };");
    expect(source).toContain('prepareLatestFollow: () => void;');
    expect(source).toContain('props.prepareLatestFollow();');
  });

  test('keeps an unstored latest view at the bottom while staged rows expand', () => {
    const source = app();
    expect(source).toContain("const followLatestUntilLayoutSettles = scrollMode === 'follow' || (scrollMode === 'restore' && !pendingAnchor && !Number.isFinite(savedTop));");
    expect(source).toContain('const resizeObserver = typeof ResizeObserver ===');
    expect(source).toContain('resizeObserver.observe(child);');
    expect(source).toContain('window.setTimeout(restorePosition, 1200);');
  });

  test('invalidates saved-position restore work before history navigation can be overwritten', () => {
    const source = app();
    expect(source).toContain('const chatViewRestoreGenerationRef = useRef(0);');
    expect(source).toContain('const chatViewRestoreDisposeRef = useRef<(() => void) | null>(null);');
    expect(source).toContain('const chatViewRestoreScrollRef = useRef<ProgrammaticChatScroll | null>(null);');
    expect(source).toContain('const cancelChatViewRestore = useCallback(() => {');
    expect(source).toContain('chatViewRestoreGenerationRef.current += 1;');
    expect(source).toContain('chatViewRestoreDisposeRef.current = null;\n    dispose?.();');
    expect(source).toContain('const restoreGeneration = ++chatViewRestoreGenerationRef.current;');
    expect(source).toContain('if (!scroller || pendingJumpMessageIdRef.current) return;');
    expect(source).toContain('if (disposed || chatViewRestoreGenerationRef.current !== restoreGeneration) return;');
    expect(source).toContain("if (sessionStreamFollowMode(chatViewportFollowRef.current, activeSessionId) === 'away') return;");
    expect(source).toContain("if (intent === 'away' && pendingScroll?.sessionId === activeSessionId) scrollLatestAfterRenderRef.current = null;");
    expect(source).toContain('cancelChatViewRestore();\n      pendingHistoryScrollAnchorRef.current = null;\n      applyStreamFollowIntent(\'away\');');
    expect(source).toContain('chatViewRestoreDisposeRef.current = disposeRestore;');
    expect(source).toContain('chatViewRestoreScrollRef.current = { sessionId: activeSessionId, scrollTop: scroller.scrollTop, generation: restoreGeneration, token };');
    expect(source).toContain('if (Math.abs(scroller.scrollTop - previousTop) <= 0.01) return;');
    expect(source).toContain('if (chatViewRestoreScrollRef.current?.token === token) chatViewRestoreScrollRef.current = null;');
    expect(source).toContain('if (chatViewRestoreScrollRef.current?.generation === restoreGeneration) chatViewRestoreScrollRef.current = null;');
    expect(source).toContain('consumeChatViewRestoreScroll={consumeChatViewRestoreScroll}');
  });
});
