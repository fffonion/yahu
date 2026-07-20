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

  test('dedupes aliased reasoning fields from Hermes API Server messages', () => {
    const parts = normalizeMessageParts('final answer', {
      reasoning: 'same provider trace',
      reasoning_content: 'same provider trace',
      thinking_content: 'different trace',
    });

    expect(parts.content).toBe('final answer');
    expect(parts.reasoning).toBe('same provider trace\ndifferent trace');
  });

  test('dedupes repeated reasoning array parts and direct fields', () => {
    const parts = normalizeMessageParts([
      { type: 'reasoning', text: 'same trace' },
      { type: 'thinking', content: 'same trace' },
      { type: 'output_text', text: 'answer' },
    ], { reasoning_content: 'same trace' });

    expect(parts.content).toBe('answer');
    expect(parts.reasoning).toBe('same trace');
  });

  test('extracts readable provider details without exposing signatures or encrypted payloads', () => {
    const parts = normalizeMessageParts('answer', {
      reasoning_details: [{ type: 'thinking', thinking: 'provider thought', signature: 'opaque-signature' }],
      codex_reasoning_items: [{
        type: 'reasoning',
        summary: [{ type: 'summary_text', text: 'provider summary' }],
        encrypted_content: 'opaque-encrypted-payload',
      }],
    });

    expect(parts.reasoning).toBe('provider thought\nprovider summary');
    expect(parts.reasoning).not.toContain('opaque-signature');
    expect(parts.reasoning).not.toContain('opaque-encrypted-payload');
  });
});
