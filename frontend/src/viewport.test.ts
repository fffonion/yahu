import { describe, expect, test } from 'bun:test';
import { visibleViewportHeight } from './viewport';

describe('visible viewport height', () => {
  test('prefers the Android visual viewport over the taller layout viewport', () => {
    expect(visibleViewportHeight({ innerHeight: 800, visualViewport: { height: 700 } })).toBe(700);
  });

  test('falls back to the layout viewport when visual viewport data is unavailable', () => {
    expect(visibleViewportHeight({ innerHeight: 800, visualViewport: null })).toBe(800);
  });
});
