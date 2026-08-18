import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('chat session message cache', () => {
  test('keeps a bounded per-session cache and restores it before the latest refresh', () => {
    const source = app();
    expect(source).toContain('const sessionMessageCacheRef = useRef<Map<string, SessionMessageCache>>(new Map());');
    expect(source).toContain('const SESSION_MESSAGE_CACHE_LIMIT = 8;');
    expect(source).toContain('while (cache.size > SESSION_MESSAGE_CACHE_LIMIT) cache.delete(cache.keys().next().value as string);');
    expect(source).toContain('const restored = restoreCachedMessageWindow(activeSessionId);');
    expect(source).toContain('setMessages(cached.messages);');
    expect(source).toContain("loadMessageWindow(activeSessionId, 'latest');");
  });

  test('updates the cached window after API pages and watch events', () => {
    const source = app();
    expect(source).toContain('cacheMessageWindow(sessionId, {');
    expect(source).toContain('cacheMessageWindow(watchedSessionId, {');
    expect(source).toContain('messages: merged.messages,');
    expect(source).toContain('messages: next,');
  });
});
