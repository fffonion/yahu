import type { ITheme } from '@xterm/xterm';

export const TERMINAL_FONT_SIZE_DEFAULT = 15;
export const TERMINAL_FONT_SIZE_MIN = 11;
export const TERMINAL_FONT_SIZE_MAX = 24;

export function clampTerminalFontSize(value: number): number {
  if (!Number.isFinite(value)) return TERMINAL_FONT_SIZE_DEFAULT;
  return Math.min(TERMINAL_FONT_SIZE_MAX, Math.max(TERMINAL_FONT_SIZE_MIN, Math.round(value)));
}

export function terminalWebSocketUrl(location: Pick<Location, 'protocol' | 'host'>, cwd = ''): string {
  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const base = `${protocol}//${location.host}/terminal/ws`;
  if (!cwd) return base;
  return `${base}?${new URLSearchParams({ cwd }).toString()}`;
}

export type TerminalModifierState = {
  ctrl: boolean;
  alt: boolean;
};

export type TerminalSpecialKey = 'escape' | 'tab' | 'up' | 'down' | 'right' | 'left';

const TERMINAL_SPECIAL_KEY_SEQUENCES: Record<TerminalSpecialKey, string> = {
  escape: '\x1b',
  tab: '\t',
  up: '\x1b[A',
  down: '\x1b[B',
  right: '\x1b[C',
  left: '\x1b[D',
};

function controlCharacter(value: string): string {
  if (value === ' ' || value === '@') return '\x00';
  if (value === '?') return '\x7f';
  const code = value.toUpperCase().charCodeAt(0);
  return code >= 65 && code <= 90 ? String.fromCharCode(code - 64) : value;
}

export function applyTerminalModifiers(data: string, modifiers: TerminalModifierState): string {
  if (!data || (!modifiers.ctrl && !modifiers.alt)) return data;
  const characters = Array.from(data);
  if (modifiers.ctrl) characters[0] = controlCharacter(characters[0]);
  const modified = characters.join('');
  return modifiers.alt ? `\x1b${modified}` : modified;
}

export function terminalSpecialKeySequence(key: TerminalSpecialKey): string {
  return TERMINAL_SPECIAL_KEY_SEQUENCES[key];
}

type TerminalThemeInput = {
  background: string;
  foreground: string;
  accent: string;
  selection: string;
  isDark: boolean;
};

export function buildTerminalTheme(input: TerminalThemeInput): ITheme {
  const shared = {
    background: input.background,
    foreground: input.foreground,
    cursor: input.accent,
    cursorAccent: input.background,
    selectionBackground: input.selection,
  };
  if (input.isDark) {
    return {
      ...shared,
      black: '#20242b',
      red: '#e26d78',
      green: '#65c466',
      yellow: '#d7b85b',
      blue: '#61a8ff',
      magenta: '#d47784',
      cyan: '#56b6c2',
      white: '#d8dee9',
      brightBlack: '#6f7782',
      brightRed: '#ff8791',
      brightGreen: '#83d982',
      brightYellow: '#ecd178',
      brightBlue: '#8fc2ff',
      brightMagenta: '#ea98a2',
      brightCyan: '#7ed2dc',
      brightWhite: '#f4f6f8',
    };
  }
  return {
    ...shared,
    black: '#24292f',
    red: '#b4232f',
    green: '#2f7d32',
    yellow: '#8a6500',
    blue: '#0969da',
    magenta: '#a13d4d',
    cyan: '#0b7285',
    white: '#e9ecef',
    brightBlack: '#57606a',
    brightRed: '#cf3342',
    brightGreen: '#418c45',
    brightYellow: '#9d7600',
    brightBlue: '#218bff',
    brightMagenta: '#bd5665',
    brightCyan: '#168799',
    brightWhite: '#ffffff',
  };
}
