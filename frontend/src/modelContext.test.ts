import { describe, expect, test } from 'bun:test';
import { fallbackContextWindowForModel, latestMessageProviderForModel, resolvePreferredModelProvider, selectModelOption } from './modelContext';

describe('model context window fallback', () => {
  test('uses the Codex GPT-5.6 context when API session provider metadata is present', () => {
    expect(fallbackContextWindowForModel('gpt-5.6-sol', 'openai-codex')).toBe(272_000);
    expect(fallbackContextWindowForModel('gpt-5.6-sol')).toBe(272_000);
  });

  test('keeps a known API session provider when a stale catalog only has another provider', () => {
    const options = [{ id: 'gpt-5.6-sol', provider: 'openrouter' }];
    expect(resolvePreferredModelProvider(options, 'gpt-5.6-sol', 'openai-codex')).toBe('openai-codex');
    expect(resolvePreferredModelProvider(options, 'gpt-5.6-sol')).toBe('openrouter');
  });

  test('uses the latest API message provider as a session fallback for the same model', () => {
    const messages = [
      { model: 'gpt-5.6-sol', provider: 'openrouter' },
      { model: 'other-model', provider: 'custom:local' },
      { model: 'gpt-5.6-sol', provider: 'openai-codex' },
    ];
    expect(latestMessageProviderForModel(messages, 'gpt-5.6-sol')).toBe('openai-codex');
    expect(latestMessageProviderForModel(messages, 'missing-model')).toBe('');
  });

  test('chooses the smallest catalog context across providers when provider is absent', () => {
    const options = [
      { id: 'gpt-5.6-sol', provider: 'openrouter', contextLength: 1_047_576 },
      { id: 'gpt-5.6-sol', provider: 'openai-codex', contextLength: 272_000 },
    ];
    expect(selectModelOption(options, 'gpt-5.6-sol')?.provider).toBe('openai-codex');
    expect(selectModelOption(options, 'gpt-5.6-sol', 'openrouter')?.provider).toBe('openrouter');
    expect(resolvePreferredModelProvider(options, 'gpt-5.6-sol')).toBe('openai-codex');
  });

  test('keeps established fallbacks and the conservative unknown default', () => {
    expect(fallbackContextWindowForModel('gpt-5.4', 'openai-codex')).toBe(1_047_576);
    expect(fallbackContextWindowForModel('claude-sonnet-4', 'anthropic')).toBe(200_000);
    expect(fallbackContextWindowForModel('unknown-model', 'custom:local')).toBe(128_000);
  });
});
