import { describe, expect, test } from 'bun:test';
import { streamGraphemes } from './streamGraphemes';

describe('streamGraphemes', () => {
  test('keeps Unicode grapheme clusters as one animated character', () => {
    expect(streamGraphemes('A👨‍👩‍👧‍👦e\u0301')).toEqual(['A', '👨‍👩‍👧‍👦', 'e\u0301']);
  });
});
