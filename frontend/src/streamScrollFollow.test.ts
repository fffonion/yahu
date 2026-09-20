import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('streaming chat scroll follow', () => {
  test('prioritizes latest follow over a saved history anchor', () => {
    const app = source();
    const followMode = app.indexOf('if (followLatestUntilLayoutSettles) {');
    const anchorRestore = app.indexOf('if (pendingAnchor && restoreMessageScrollAnchor(scroller, pendingAnchor)) {');

    expect(followMode).toBeGreaterThan(-1);
    expect(anchorRestore).toBeGreaterThan(-1);
    expect(followMode).toBeLessThan(anchorRestore);
  });

  test('marks the viewport for follow on every local stream update', () => {
    const app = source();
    const streamFn = app.indexOf('const scrollWithStream = () => {');
    const streamFnEnd = app.indexOf('\n      };', streamFn);
    const streamFnSource = app.slice(streamFn, streamFnEnd);

    expect(streamFn).toBeGreaterThan(-1);
    expect(streamFnSource).toContain("if (followState.sessionId !== sessionId || followState.mode !== 'follow') return;");
    expect(streamFnSource).toContain("scrollLatestAfterRenderRef.current = { sessionId, mode: 'follow' };");
    expect(streamFnSource).toContain('requestAnimationFrame(scrollToLatestIfFollowing);');
  });

  test('keeps reasoning growth on the same streaming follow path', () => {
    const app = source();
    const reasoning = app.indexOf("if (event === 'reasoning.delta'");
    const reasoningEnd = app.indexOf('\n            }', reasoning);
    const reasoningSource = app.slice(reasoning, reasoningEnd);

    expect(reasoning).toBeGreaterThan(-1);
    expect(reasoningSource).toContain('scrollWithStream();');
  });

  test('uses live user intent for watched streams without consulting persisted scroll', () => {
    const app = source();
    expect(app).toContain("const followMode = followState.sessionId === watchedSessionId ? followState.mode : 'auto';");
    expect(app).toContain("const wasNearBottom = followMode === 'follow' || (followMode === 'auto' && !!chatScrollRef.current && isNearBottom(chatScrollRef.current));");
    expect(app).not.toContain('const viewportMatchesSaved = !Number.isFinite(savedScrollTop)');
  });

  test('cancels immediate and delayed follow as soon as the user scrolls upward', () => {
    const app = source();
    expect(app).toContain('if (e.deltaY < 0) {');
    expect(app).toContain('previousChatScrollTopRef.current = props.chatScrollRef.current?.scrollTop ?? null;');
    expect(app).toContain("if (intent === 'away' && pendingScroll?.sessionId === activeSessionId && pendingScroll.mode === 'follow') scrollLatestAfterRenderRef.current = null;");
    expect(app).toContain("if (followState.sessionId === activeSessionId && followState.mode === 'away') return;");
    expect(app).toContain("if (followState.sessionId !== sessionId || followState.mode !== 'follow') return;");
  });

  test('scopes pending render scroll work to its originating session', () => {
    const app = source();
    expect(app).toContain("const scrollLatestAfterRenderRef = useRef<{ sessionId: string; mode: 'follow' | 'restore' } | null>(null);");
    expect(app).toContain('const pendingScroll = scrollLatestAfterRenderRef.current;');
    expect(app).toContain("const scrollMode = pendingScroll?.sessionId === activeSessionId ? pendingScroll.mode : false;");
  });

  test('invalidates queued layout and latest-jump scroll callbacks', () => {
    const app = source();
    expect(app).toContain('let disposed = false;');
    expect(app).toContain('if (disposed) return;');
    expect(app).toContain('if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame);');
    expect(app).toContain('const latestScrollGenerationRef = useRef(0);');
    expect(app).toContain('const latestScrollHandlesRef = useRef<{ frame: number | null; timers: number[] }>({ frame: null, timers: [] });');
    expect(app).toContain('cancelLatestViewportScroll();');
    expect(app).toContain('const jumpSessionId = props.activeSessionId;');
    expect(app).toContain('if (latestScrollGenerationRef.current !== generation || latestScrollSessionRef.current !== jumpSessionId) return;');
    expect(app).toContain("!(followState.sessionId === sessionId && followState.mode === 'away')");
  });

});
