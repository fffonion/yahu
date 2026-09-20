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
    expect(source).toContain('const wasNearBottom = !!chatScrollRef.current && isNearBottom(chatScrollRef.current);');
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
    expect(source).toContain('const savedAnchor = readChatViewAnchor(activeSessionId);');
    expect(source).toContain('pendingHistoryScrollAnchorRef.current = savedAnchor ? { id: savedAnchor.id, topOffset: savedAnchor.topOffset } : null;');
    expect(source).toContain("scrollLatestAfterRenderRef.current = 'restore';");
    expect(source).toContain("if (pendingAnchor && restoreMessageScrollAnchor(scroller, pendingAnchor)) {");
    expect(source).toContain('pendingHistoryScrollAnchorRef.current = null;');
    expect(source).toContain('scroller.scrollTop = Math.min(Math.max(0, Number(savedTop)), Math.max(0, scroller.scrollHeight - scroller.clientHeight));');
    expect(source).toContain('window.setTimeout(restorePosition, 300);');
    expect(source).toContain('scroller.scrollTop = scroller.scrollHeight;');
  });

  test('marks live updates and explicit latest jumps as follow actions', () => {
    const source = app();
    expect(source).toContain("scrollLatestAfterRenderRef.current = 'follow';");
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
});
