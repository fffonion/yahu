import { describe, expect, test } from 'bun:test';
import { shouldRenderMessage } from './messageVisibility';

describe('chat message visibility', () => {
  test('hides completed assistant messages that have no visible content', () => {
    expect(shouldRenderMessage({ role: 'assistant', content: '', pending: false })).toBe(false);
    expect(shouldRenderMessage({ role: 'assistant', content: '   \n\t', pending: false })).toBe(false);
  });

  test('keeps pending assistant placeholders and messages with content', () => {
    expect(shouldRenderMessage({ role: 'assistant', content: '', pending: true })).toBe(true);
    expect(shouldRenderMessage({ role: 'assistant', content: 'hello', pending: false })).toBe(true);
  });

  test('keeps reasoning-only assistant messages only when reasoning is visible', () => {
    const msg = { role: 'assistant', content: '', reasoning: 'hidden trace', pending: false };
    expect(shouldRenderMessage(msg, false)).toBe(false);
    expect(shouldRenderMessage(msg, true)).toBe(true);
  });
});
