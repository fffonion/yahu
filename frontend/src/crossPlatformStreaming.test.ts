import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('cross-platform session streaming watcher', () => {
  test('watch events merge into chat messages, replace streaming updates, and show a pending assistant card for remote user turns', () => {
    const app = source();

    expect(app).toContain("const OTHER_PLATFORM_PENDING_ID = 'other-platform-pending';");
    expect(app).toContain('function mergeWatchedMessage(prev: ChatMessage[], msg: ChatMessage): ChatMessage[]');
    expect(app).toContain('if (prev.some((m) => m.id === msg.id)) return prev.map((m) => m.id === msg.id ? { ...m, ...msg } : m);');
    expect(app).toContain("msg.role === 'user'");
    expect(app).toContain("msg.role === 'assistant'");
    expect(app).toContain("m.id.startsWith('assistant_') && m.content === msg.content");
    expect(app).toContain('setMessages((prev) => {');
    expect(app).toContain('const next = mergeWatchedMessage(prev, msg);');
  });
});
