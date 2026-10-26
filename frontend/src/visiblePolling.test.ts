import { describe, expect, test } from 'bun:test';
import { startVisiblePolling } from './visiblePolling';
import { readFileSync } from 'node:fs';

function environment(hidden = false) {
  let listener = () => {};
  let tick: (() => void) | null = null;
  const source = { hidden, addEventListener: (_: string, callback: () => void) => { listener = callback; }, removeEventListener: (_: string, callback: () => void) => { if (listener === callback) listener = () => {}; } };
  const clock = { setInterval: (callback: () => void) => { tick = callback; return 1; }, clearInterval: () => { tick = null; } };
  return { source, clock, tick: () => tick?.(), active: () => tick !== null, visibility: (hidden: boolean) => { source.hidden = hidden; listener(); } };
}
const flush = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); };

describe('visible polling', () => {
  test('releases the in-flight guard after rejected and synchronous tasks', async () => {
    const env = environment();
    let calls = 0;
    const stop = startVisiblePolling(() => {
      calls++;
      if (calls === 1) return Promise.reject(new Error('temporary failure'));
      if (calls === 2) throw new Error('synchronous failure');
    }, 1500, false, env.source, env.clock);
    expect(calls).toBe(0);
    env.tick(); await flush();
    env.tick(); await flush();
    env.tick(); await flush();
    expect(calls).toBe(3);
    stop();
    expect(env.active()).toBe(false);
  });
  test('uses visibility-aware polling for catalog, sessions and stream status', () => {
    const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
    expect(app).toContain("from './visiblePolling'");
    expect(app.match(/startVisiblePolling\(/g)?.length).toBe(3);
    expect(app).toContain("if (mode !== 'chat' || !activeSessionId || activeSessionId === DRAFT_SESSION_ID) return;");
  });
  test('stops background timers and refreshes immediately on return without overlap', async () => {
    const env = environment(true);
    let calls = 0;
    let finish!: () => void;
    const stop = startVisiblePolling(() => { calls++; return new Promise<void>((resolve) => { finish = resolve; }); }, 1500, true, env.source, env.clock);
    expect(env.active()).toBe(false);
    expect(calls).toBe(0);
    env.visibility(false);
    expect(calls).toBe(1);
    env.tick();
    expect(calls).toBe(1);
    finish(); await flush();
    env.tick();
    expect(calls).toBe(2);
    env.visibility(true);
    expect(env.active()).toBe(false);
    finish(); await flush();
    env.visibility(false);
    expect(calls).toBe(3);
    stop();
    finish(); await flush();
    env.visibility(true); env.visibility(false); env.tick();
    expect(env.active()).toBe(false);
    expect(calls).toBe(3);
  });
});
