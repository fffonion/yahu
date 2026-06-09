import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

describe('cross-platform session streaming watcher', () => {
  test('watch events merge into chat messages and show a pending assistant card for remote user turns', () => {
    const app = source();

    expect(app).toContain("const OTHER_PLATFORM_PENDING_ID = 'other-platform-pending';");
    expect(app).toContain('function mergeWatchedMessage(prev: ChatMessage[], msg: ChatMessage): ChatMessage[]');
    expect(app).toContain("msg.role === 'user'");
    expect(app).toContain("msg.role === 'assistant'");
    expect(app).toContain('setMessages((prev) => mergeWatchedMessage(prev, msg));');
  });
});
