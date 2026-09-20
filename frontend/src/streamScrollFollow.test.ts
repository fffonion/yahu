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
    expect(streamFnSource).toContain("scrollLatestAfterRenderRef.current = 'follow';");
    expect(streamFnSource).toContain('isNearBottom(chatScrollRef.current, 220)');
  });
});
