import { describe, expect, test } from 'bun:test';
import { isTextEntryElement, resumedViewportHeight, visibleViewportHeight } from './viewport';

describe('visible viewport height', () => {
  test('prefers the Android visual viewport over the taller layout viewport', () => {
    expect(visibleViewportHeight({ innerHeight: 800, visualViewport: { height: 700 } })).toBe(700);
  });

  test('falls back to the layout viewport when visual viewport data is unavailable', () => {
    expect(visibleViewportHeight({ innerHeight: 800, visualViewport: null })).toBe(800);
  });

  test('uses the layout viewport when visual viewport is stale after resume', () => {
    expect(resumedViewportHeight({ innerHeight: 800, visualViewport: { height: 700 } })).toBe(800);
    expect(resumedViewportHeight({ innerHeight: 700, clientHeight: 800, visualViewport: { height: 700 } })).toBe(800);
    expect(resumedViewportHeight({ innerHeight: 800, visualViewport: { height: 820 } })).toBe(820);
  });

  test('recognizes focused controls that can open the software keyboard', () => {
    expect(isTextEntryElement({ tagName: 'input' })).toBe(true);
    expect(isTextEntryElement({ tagName: 'TEXTAREA' })).toBe(true);
    expect(isTextEntryElement({ tagName: 'select' })).toBe(true);
    expect(isTextEntryElement({ isContentEditable: true })).toBe(true);
    expect(isTextEntryElement({ tagName: 'button' })).toBe(false);
    expect(isTextEntryElement(null)).toBe(false);
  });
});
