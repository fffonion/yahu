import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { matchesProgrammaticChatScroll, sessionStreamFollowMode, shouldAutoLoadOlderForHiddenHistory, shouldLoadOlderFromWheel, shouldSyncMinimapToLatest, streamFollowIntentAfterScroll } from './chatHistoryScroll';

const appSource = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const cssSource = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat history scroll triggers', () => {
  test('wheel-up at the top requests older history even when hidden tool messages leave no scroll delta', () => {
    expect(shouldLoadOlderFromWheel({ scrollTop: 0, scrollHeight: 420, clientHeight: 640 }, -24, true, false)).toBe(true);
  });

  test('wheel-up near the top requests older history after hidden rows shrink the scroll range', () => {
    expect(shouldLoadOlderFromWheel({ scrollTop: 42, scrollHeight: 720, clientHeight: 640 }, -24, true, false)).toBe(true);
  });

  test('does not request older history for wheel-down, missing older pages, or active load', () => {
    const atTop = { scrollTop: 0, scrollHeight: 420, clientHeight: 640 };
    expect(shouldLoadOlderFromWheel(atTop, 18, true, false)).toBe(false);
    expect(shouldLoadOlderFromWheel(atTop, -18, false, false)).toBe(false);
    expect(shouldLoadOlderFromWheel(atTop, -18, true, true)).toBe(false);
  });

  test('auto-loads older history when hidden tool rows leave the chat scroller too short to scroll', () => {
    expect(shouldAutoLoadOlderForHiddenHistory({ scrollTop: 0, scrollHeight: 420, clientHeight: 640 }, true, false)).toBe(true);
    expect(shouldAutoLoadOlderForHiddenHistory({ scrollTop: 0, scrollHeight: 900, clientHeight: 640 }, true, false)).toBe(false);
    expect(shouldAutoLoadOlderForHiddenHistory({ scrollTop: 0, scrollHeight: 420, clientHeight: 640 }, false, false)).toBe(false);
    expect(shouldAutoLoadOlderForHiddenHistory({ scrollTop: 0, scrollHeight: 420, clientHeight: 640 }, true, true)).toBe(false);
  });

  test('user upward intent disables stream follow even inside the near-bottom threshold', () => {
    expect(streamFollowIntentAfterScroll(100, { scrollTop: 92, scrollHeight: 480, clientHeight: 369 })).toBe('away');
    expect(streamFollowIntentAfterScroll(100, { scrollTop: 99.75, scrollHeight: 480, clientHeight: 369 })).toBe('away');
    expect(streamFollowIntentAfterScroll(92, { scrollTop: 111, scrollHeight: 480, clientHeight: 369 })).toBe('follow');
    expect(streamFollowIntentAfterScroll(92, { scrollTop: 110, scrollHeight: 480, clientHeight: 369 })).toBe(null);
    expect(streamFollowIntentAfterScroll(92, { scrollTop: 101, scrollHeight: 480, clientHeight: 369 })).toBe(null);
    expect(streamFollowIntentAfterScroll(null, { scrollTop: 111, scrollHeight: 480, clientHeight: 369 })).toBe(null);
    expect(streamFollowIntentAfterScroll(111, { scrollTop: 111, scrollHeight: 480, clientHeight: 369 })).toBe(null);
    expect(streamFollowIntentAfterScroll(92, { scrollTop: 100, scrollHeight: 900, clientHeight: 369 })).toBe(null);
  });

  test('minimap latest sync obeys the shared session follow mode', () => {
    const atBottom = { scrollTop: 111, scrollHeight: 480, clientHeight: 369 };
    const nearBottom = { scrollTop: 101, scrollHeight: 480, clientHeight: 369 };
    const farFromBottom = { scrollTop: 100, scrollHeight: 900, clientHeight: 369 };
    expect(shouldSyncMinimapToLatest('away', atBottom)).toBe(false);
    expect(shouldSyncMinimapToLatest('away', nearBottom)).toBe(false);
    expect(shouldSyncMinimapToLatest('follow', farFromBottom)).toBe(true);
    expect(shouldSyncMinimapToLatest('auto', nearBottom)).toBe(true);
    expect(shouldSyncMinimapToLatest('auto', farFromBottom)).toBe(false);
  });

  test('follow intent cannot leak between sessions', () => {
    expect(sessionStreamFollowMode({ sessionId: 'session-a', mode: 'away' }, 'session-a')).toBe('away');
    expect(sessionStreamFollowMode({ sessionId: 'session-a', mode: 'away' }, 'session-b')).toBe('auto');
    expect(sessionStreamFollowMode({ sessionId: 'session-a', mode: 'follow' }, 'session-b')).toBe('auto');
  });

  test('restore-generated scroll events match only their recorded session and target', () => {
    const target = { sessionId: 'session-b', scrollTop: 245.5, generation: 3, token: 7 };
    expect(matchesProgrammaticChatScroll(target, 'session-b', 245.5)).toBe(true);
    expect(matchesProgrammaticChatScroll(target, 'session-b', 245.505)).toBe(true);
    expect(matchesProgrammaticChatScroll(target, 'session-b', 245.6)).toBe(false);
    expect(matchesProgrammaticChatScroll(target, 'session-b', 248)).toBe(false);
    expect(matchesProgrammaticChatScroll(target, 'session-a', 245.5)).toBe(false);
    expect(matchesProgrammaticChatScroll(null, 'session-b', 245.5)).toBe(false);
  });

  test('older history loading preserves the current message anchor instead of height-delta jumping', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const pendingHistoryScrollAnchorRef = useRef<{ sessionId: string; anchor: MessageScrollAnchor } | null>(null);');
    expect(app).toContain("pendingHistoryScrollAnchorRef.current = anchor ? { sessionId, anchor } : null;");
    expect(app).toContain('restoreMessageScrollAnchor(scroller, anchor);');
    expect(css).toContain('.chat-scroll{position:relative;overflow:auto;');
    expect(css).toContain('.history-loading{position:absolute;top:10px;left:50%;transform:translateX(-50%);');
  });

  test('older history requests backfill to a user or system boundary before merging compact detail turns', () => {
    const app = appSource();
    expect(app).toContain("import { backfillOlderChunkToTurnBoundary, normalizeChatHistoryChunk");
    expect(app).toContain('backfillOlderChunkToTurnBoundary({');
    expect(app).toContain('rawWindowLimit: RAW_MESSAGE_WINDOW');
  });
});
