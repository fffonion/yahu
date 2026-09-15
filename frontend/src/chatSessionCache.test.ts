import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('chat session message cache', () => {
  test('keeps a bounded per-session cache and restores it before the latest refresh', () => {
    const source = app();
    expect(source).toContain('const sessionMessageCacheRef = useRef<Map<string, SessionMessageCache>>(new Map());');
    expect(source).toContain('const SESSION_MESSAGE_CACHE_LIMIT = 2;');
    expect(source).toContain('while (cache.size > SESSION_MESSAGE_CACHE_LIMIT) cache.delete(cache.keys().next().value as string);');
    expect(source).toContain('const restored = hydrated || restoreCachedMessageWindow(activeSessionId);');
    expect(source).toContain('setMessages(cached.messages);');
    expect(source).toContain("loadMessageWindow(activeSessionId, 'latest', restored ? undefined : savedAnchor?.id);");
  });

  test('caches the outgoing active window during a session switch only', () => {
    const source = app();
    expect(source).toContain('const cacheCurrentSessionWindow = useCallback(() => {');
    expect(source).toContain('cacheMessageWindow(sessionId, {');
    expect(source).toContain('cacheCurrentSessionWindow();');
    expect(source).toContain('const cached = sessionMessageCacheRef.current.get(sessionId);');
    expect(source).toContain('cache.delete(sessionId);');
    expect(source).not.toContain('cacheMessageWindow(watchedSessionId, {');
    expect(source.match(/cacheMessageWindow\(/g)?.length).toBe(1);
  });

  test('does not retain a resize observer target for every transcript row', () => {
    const source = app();
    expect(source).toContain('resizeObserver.observe(scroller);');
    expect(source).not.toContain("scroller.querySelectorAll<HTMLElement>('[data-message-id]').forEach((row) => observer.observe(row));");
  });
});
