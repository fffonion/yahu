import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

const sliceBetween = (text: string, start: string, end: string) => {
  const from = text.indexOf(start);
  const to = text.indexOf(end, from);
  return text.slice(from, to === -1 ? undefined : to);
};

describe('chat viewport user takeover', () => {
  test('a real user scroll cancels queued viewport restoration', () => {
    const app = source();
    const onScroll = sliceBetween(app, 'const onScroll = (e: React.UIEvent<HTMLElement>) => {', 'const onWheel = (e: React.WheelEvent<HTMLElement>) => {');
    expect(onScroll.length).toBeGreaterThan(0);
    // Only a genuine (non-restore) move counts, so the restore baseline cannot cancel itself.
    expect(onScroll).toContain('const userScrolled = !restoredScroll && previousScroll?.sessionId === props.activeSessionId && Math.abs(el.scrollTop - previousScroll.scrollTop) > 0.5;');
    expect(onScroll).toContain('if (userScrolled) props.onChatViewTookOver();');
  });

  test('takeover marks the session and drops pending restore handles', () => {
    const app = source();
    expect(app).toContain("const chatViewTookOverRef = useRef<string>('');");
    const takeover = sliceBetween(app, 'const markChatViewTookOver = useCallback(() => {', 'const showToast = useCallback(');
    expect(takeover).toContain('chatViewTookOverRef.current = activeSessionId;');
    expect(takeover).toContain('cancelChatViewRestore();');
  });

  test('takeover suppresses saved position and stored anchor restoration, but keeps follow', () => {
    const app = source();
    const restore = sliceBetween(app, 'useLayoutEffect(() => {', 'const scroller = chatScrollRef.current;');
    expect(app).toContain('const tookOver = chatViewTookOverRef.current === activeSessionId;');
    expect(app).toContain('const savedTop = tookOver ? null : readChatViewPosition(activeSessionId);');
    expect(app).toContain('!followLatestUntilLayoutSettles && !ownAnchor) return;');
    // The scroll-to-bottom fallback must not fire after takeover without a saved position.
    expect(app).toContain('} else if (!tookOver) {');
    expect(app).toContain('scroller.scrollTop = scroller.scrollHeight;');
  });

  test('switching sessions clears the takeover marker', () => {
    const app = source();
    expect(app).toContain("useEffect(() => { chatViewTookOverRef.current = ''; writeChatViewState(activeSessionId); }, [activeSessionId]);");
  });

  test('chat view wires the takeover callback through to ChatMain', () => {
    const app = source();
    expect(app).toContain('onChatViewTookOver: () => void;');
    expect(app).toContain('onChatViewTookOver={markChatViewTookOver}');
  });
});
