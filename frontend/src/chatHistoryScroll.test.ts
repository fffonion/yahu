import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { shouldAutoLoadOlderForHiddenHistory, shouldLoadOlderFromWheel } from './chatHistoryScroll';

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

  test('older history loading preserves the current message anchor instead of height-delta jumping', () => {
    const app = appSource();
    const css = cssSource();
    expect(app).toContain('const pendingHistoryScrollAnchorRef = useRef<MessageScrollAnchor | null>(null);');
    expect(app).toContain("if (direction === 'older') pendingHistoryScrollAnchorRef.current = captureMessageScrollAnchor(scroller);");
    expect(app).toContain('restoreMessageScrollAnchor(scroller, anchor);');
    expect(app).not.toContain('const oldHeight = scroller?.scrollHeight || 0;');
    expect(app).not.toContain('scroller.scrollTop += scroller.scrollHeight - oldHeight;');
    expect(css).toContain('.chat-scroll{position:relative;overflow:auto;');
    expect(css).toContain('.history-loading{position:absolute;top:10px;left:50%;transform:translateX(-50%);');
  });
});
