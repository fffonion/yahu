import { describe, expect, test } from 'bun:test';
import { applyTerminalModifiers, buildTerminalTheme, clampTerminalFontSize, terminalSpecialKeySequence, terminalWebSocketUrl } from './terminalSupport';

describe('web terminal helpers', () => {
  test('builds a same-origin websocket URL with an optional encoded cwd', () => {
    expect(terminalWebSocketUrl({ protocol: 'http:', host: '127.0.0.1:9642' })).toBe('ws://127.0.0.1:9642/terminal/ws');
    expect(terminalWebSocketUrl({ protocol: 'https:', host: 'yahu.example' }, 'src/backend folder')).toBe('wss://yahu.example/terminal/ws?cwd=src%2Fbackend+folder');
  });

  test('clamps persisted terminal font sizes to the supported range', () => {
    expect(clampTerminalFontSize(4)).toBe(11);
    expect(clampTerminalFontSize(16)).toBe(16);
    expect(clampTerminalFontSize(99)).toBe(24);
    expect(clampTerminalFontSize(Number.NaN)).toBe(15);
  });

  test('applies one-shot Ctrl and Alt modifiers to mobile terminal input', () => {
    expect(applyTerminalModifiers('c', { ctrl: true, alt: false })).toBe('\x03');
    expect(applyTerminalModifiers('cd', { ctrl: true, alt: false })).toBe('\x03d');
    expect(applyTerminalModifiers('x', { ctrl: false, alt: true })).toBe('\x1bx');
    expect(applyTerminalModifiers('c', { ctrl: true, alt: true })).toBe('\x1b\x03');
    expect(applyTerminalModifiers('plain', { ctrl: false, alt: false })).toBe('plain');
  });

  test('maps mobile terminal special keys to PTY escape sequences', () => {
    expect(terminalSpecialKeySequence('escape')).toBe('\x1b');
    expect(terminalSpecialKeySequence('tab')).toBe('\t');
    expect(terminalSpecialKeySequence('up')).toBe('\x1b[A');
    expect(terminalSpecialKeySequence('down')).toBe('\x1b[B');
    expect(terminalSpecialKeySequence('right')).toBe('\x1b[C');
    expect(terminalSpecialKeySequence('left')).toBe('\x1b[D');
    expect(terminalSpecialKeySequence('backspace')).toBe('\x7f');
    expect(terminalSpecialKeySequence('delete')).toBe('\x1b[3~');
    expect(terminalSpecialKeySequence('insert')).toBe('\x1b[2~');
    expect(terminalSpecialKeySequence('home')).toBe('\x1b[H');
    expect(terminalSpecialKeySequence('end')).toBe('\x1b[F');
    expect(terminalSpecialKeySequence('pageUp')).toBe('\x1b[5~');
    expect(terminalSpecialKeySequence('pageDown')).toBe('\x1b[6~');
    expect(terminalSpecialKeySequence('f1')).toBe('\x1bOP');
    expect(terminalSpecialKeySequence('f12')).toBe('\x1b[24~');
  });

  test('maps Yahu surfaces and conventional ANSI colors without a purple-dominant palette', () => {
    const dark = buildTerminalTheme({
      background: '#1e1e1e',
      foreground: '#d4d4d4',
      accent: '#4f8cff',
      selection: '#264f78',
      isDark: true,
    });
    const light = buildTerminalTheme({
      background: '#ffffff',
      foreground: '#24292f',
      accent: '#0969da',
      selection: '#b6d7ff',
      isDark: false,
    });

    expect(dark.background).toBe('#1e1e1e');
    expect(dark.foreground).toBe('#d4d4d4');
    expect(dark.blue).toBe('#61a8ff');
    expect(dark.green).toBe('#65c466');
    expect(dark.magenta).toBe('#d47784');
    expect(light.blue).toBe('#0969da');
    expect(light.green).toBe('#2f7d32');
    expect(light.magenta).toBe('#a13d4d');
    expect(Object.values(dark).join(' ').toLowerCase()).not.toContain('purple');
  });
});
