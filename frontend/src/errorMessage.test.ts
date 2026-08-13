import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { errorMessage, isAbortError } from './errorMessage';

describe('errorMessage', () => {
  test('reads Error and error-shaped object messages', () => {
    expect(errorMessage(new Error('request failed'))).toBe('request failed');
    expect(errorMessage({ message: 'upstream unavailable' })).toBe('upstream unavailable');
  });

  test('uses string values and a fallback for empty errors', () => {
    expect(errorMessage('plain failure')).toBe('plain failure');
    expect(errorMessage(503)).toBe('503');
    expect(errorMessage({ code: 503 })).toBe('[object Object]');
    expect(errorMessage(null, 'unknown failure')).toBe('unknown failure');
    expect(errorMessage({ message: '' }, 'unknown failure')).toBe('unknown failure');
  });

  test('never lets hostile error objects escape the error handler', () => {
    const hostile = new Proxy({}, {
      get: () => { throw new Error('get trap'); },
      getPrototypeOf: () => { throw new Error('prototype trap'); },
      has: () => { throw new Error('has trap'); },
    });

    expect(errorMessage(hostile, 'unknown failure')).toBe('unknown failure');
    expect(isAbortError(hostile)).toBe(false);
  });


  test('recognizes browser-style abort errors without assuming Error instances', () => {
    expect(isAbortError({ name: 'AbortError' })).toBe(true);
    expect(isAbortError(new Error('request failed'))).toBe(false);
    expect(isAbortError(null)).toBe(false);
  });
});
