import { describe, expect, test } from 'bun:test';
import { currentModelDisplayOption, providerDisplayName } from './modelDisplay';

describe('model provider display', () => {
  test('shows only the model in the active selector label even when an API provider is available', () => {
    const option = currentModelDisplayOption('gpt-5.5', [
      { id: 'gpt-5.5', label: 'GitHub Copilot · gpt-5.5', provider: 'copilot' },
    ], 'openai-codex');

    expect(option.label).toBe('gpt-5.5');
    expect(option.provider).toBeUndefined();
  });

  test('does not synthesize a provider label when the exact model alias is not in inventory', () => {
    const option = currentModelDisplayOption('minimax-m3', [], 'minimax');

    expect(option.label).toBe('minimax-m3');
    expect(option.provider).toBeUndefined();
  });

  test('formats known provider ids without deriving provider from model text', () => {
    expect(providerDisplayName('openai-codex')).toBe('Codex');
    expect(providerDisplayName('minimax')).toBe('MiniMax');
  });

  test('does not infer the selected session provider from duplicate inventory model ids', () => {
    const option = currentModelDisplayOption('gpt-5.5', [
      { id: 'gpt-5.5', label: 'GitHub Copilot · gpt-5.5', provider: 'copilot' },
    ], '');

    expect(option.label).toBe('gpt-5.5');
    expect(option.provider).toBeUndefined();
  });
});
