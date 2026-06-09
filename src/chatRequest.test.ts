import { describe, expect, test } from 'bun:test';
import { buildChatRequestBody } from './chatRequest';

describe('buildChatRequestBody', () => {
  test('sends reasoning effort as a parameter, not an instruction', () => {
    const body = buildChatRequestBody('hello', 'gpt-5.5', 'medium');
    expect(body).toEqual({ input: 'hello', model: 'gpt-5.5', reasoning_effort: 'medium' });
    expect('instructions' in body).toBe(false);
  });

  test('carries provider when the model selector supplies one', () => {
    const body = buildChatRequestBody('hello', 'MiniMax-M3', 'medium', 'minimax-cn');
    expect(body).toEqual({ input: 'hello', model: 'MiniMax-M3', provider: 'minimax-cn', reasoning_effort: 'medium' });
  });
});
