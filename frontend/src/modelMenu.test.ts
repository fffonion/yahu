import { describe, expect, test } from 'bun:test';
import { modelMenuLabel, modelTriggerLabel, orderedModelOptions } from './modelDisplay';

describe('model menu ordering and labels', () => {
  const options = [
    { id: 'zeta', label: 'zeta', provider: 'openrouter' },
    { id: 'deepseek/deepseek-v4-flash', label: 'deepseek/deepseek-v4-flash', provider: 'deepseek' },
    { id: 'mixture-of-agents', label: 'mixture-of-agents', provider: 'openrouter' },
    { id: 'alpha', label: 'alpha', provider: 'openai' },
    { id: 'beta', label: 'beta', provider: 'openrouter' },
    { id: 'gamma', label: 'gamma', provider: 'openrouter' },
  ];

  test('puts the selected model first, then its provider, then alphabetical providers, with mixture of agents last', () => {
    const ordered = orderedModelOptions(options, 'beta', 'openrouter');
    expect(ordered.map((item) => item.id)).toEqual([
      'beta',
      'gamma',
      'zeta',
      'deepseek/deepseek-v4-flash',
      'alpha',
      'mixture-of-agents',
    ]);
  });

  test('shows only the slash suffix on mobile and the full model on desktop', () => {
    expect(modelTriggerLabel('deepseek/deepseek-v4-flash', true)).toBe('deepseek-v4-flash');
    expect(modelTriggerLabel('deepseek/deepseek-v4-flash', false)).toBe('deepseek/deepseek-v4-flash');
  });

  test('fills the current provider when the current option lacks provider metadata', () => {
    const [current] = orderedModelOptions([{ id: 'gpt-5.6-luna', label: 'gpt-5.6-luna' }], 'gpt-5.6-luna', 'openai-codex');
    expect(current.provider).toBe('openai-codex');
    expect(modelMenuLabel(current)).toBe('Codex - gpt-5.6-luna');
  });
});
