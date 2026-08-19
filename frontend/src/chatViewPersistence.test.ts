import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('chat view persistence', () => {
  test('stores the last session and per-session scroll positions in localStorage', () => {
    const source = app();
    expect(source).toContain("const CHAT_VIEW_STATE_KEY = 'yahu.chat.view.v1';");
    expect(source).toContain('const readChatViewState = (): { lastSessionId: string; positions: Record<string, number>; anchors: Record<string, string> }');
    expect(source).toContain('state.positions[sessionId] = Math.max(0, Number(scrollTop));');
    expect(source).toContain('state.anchors[sessionId] = anchorId;');
    expect(source).toContain('writeChatViewState(props.activeSessionId, el.scrollTop, captureMessageScrollAnchor(el)?.id);');
    expect(source).toContain('const viewportMatchesSaved = !Number.isFinite(savedScrollTop) || Math.abs((chatScrollRef.current?.scrollTop || 0) - Number(savedScrollTop)) <= 2;');
  });

  test('uses the stored session when the chat route has no explicit session', () => {
    const source = app();
    expect(source).toContain("initialRoute.sessionId || initialChatView.lastSessionId || ''");
    expect(source).toContain('const initialChatView = readChatViewState();');
    expect(source).toContain('clearLastChatViewSession();');
  });

  test('restores a saved position after the latest page renders and otherwise goes latest', () => {
    const source = app();
    expect(source).toContain('const savedAnchorId = readChatViewAnchor(activeSessionId);');
    expect(source).toContain("loadMessageWindow(activeSessionId, 'latest', restored ? undefined : savedAnchorId);");
    expect(source).toContain("params.set('around', around);");
    expect(source).toContain("const scrollMode = scrollLatestAfterRenderRef.current;");
    expect(source).toContain("if (scrollMode === 'follow') {");
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
});
