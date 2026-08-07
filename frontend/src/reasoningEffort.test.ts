import { describe, expect, test } from 'bun:test';
import { normalizeReasoningEffort, REASONING_EFFORTS, sessionReasoningEffort } from './reasoningEffort';

describe('reasoning effort selection', () => {
  test('exposes every supported reasoning effort', () => {
    expect(REASONING_EFFORTS).toEqual(['minimal', 'low', 'medium', 'high', 'xhigh', 'max', 'ultra']);
  });

  test('normalizes supported values and rejects unknown values', () => {
    expect(normalizeReasoningEffort(' XHIGH ')).toBe('xhigh');
    expect(normalizeReasoningEffort('max')).toBe('max');
    expect(normalizeReasoningEffort('unsupported')).toBeNull();
  });

  test('reads the current effort from object or serialized session model config', () => {
    expect(sessionReasoningEffort({ model_config: { reasoning_config: { effort: 'ultra' } } })).toBe('ultra');
    expect(sessionReasoningEffort({ model_config: '{"reasoning_config":{"effort":"xhigh"}}' })).toBe('xhigh');
    expect(sessionReasoningEffort({ reasoning_effort: 'max', model_config: { reasoning_config: { effort: 'low' } } })).toBe('max');
    expect(sessionReasoningEffort({ model_config: { reasoning_config: { effort: 'unknown' } } })).toBeNull();
  });
});
