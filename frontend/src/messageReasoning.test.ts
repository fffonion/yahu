import { describe, expect, test } from 'bun:test';
import { normalizeMessageParts } from './messageReasoning';

describe('message reasoning normalization', () => {
  test('keeps reasoning/thinking parts separate from visible assistant content', () => {
    const parts = normalizeMessageParts([
      { type: 'reasoning', text: 'first think' },
      { type: 'output_text', text: 'final answer' },
      { type: 'thinking', content: 'second think' },
    ]);

    expect(parts.content).toBe('final answer');
    expect(parts.reasoning).toBe('first think\nsecond think');
  });

  test('extracts common API reasoning fields without duplicating them into content', () => {
    const parts = normalizeMessageParts('final answer', { reasoning_content: 'hidden chain', thinking: 'draft thought' });

    expect(parts.content).toBe('final answer');
    expect(parts.reasoning).toBe('hidden chain\ndraft thought');
  });
});
