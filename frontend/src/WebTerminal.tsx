import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { Eraser, Keyboard, Minus, Plus, RefreshCw, TerminalSquare } from 'lucide-react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import './webTerminalFont.css';
import { t } from './i18n';
import { buildTerminalTheme, clampTerminalFontSize, terminalWebSocketUrl, TERMINAL_FONT_SIZE_DEFAULT } from './terminalSupport';

const TERMINAL_FONT_SIZE_KEY = 'yahu-terminal-font-size';
const TERMINAL_FONT_FAMILY = '"SauceCodePro Nerd Font Mono", "SauceCodePro NFM", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace';
const TERMINAL_FONT_LOAD = `400 ${TERMINAL_FONT_SIZE_DEFAULT}px "SauceCodePro Nerd Font Mono"`;

type ConnectionState = 'connecting' | 'connected' | 'disconnected' | 'error';

type WebTerminalProps = {
  active?: boolean;
  cwd?: string;
  theme: string;
  headerActions?: ReactNode;
};

function cssColor(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function terminalThemeFromDocument() {
  return buildTerminalTheme({
    background: cssColor('--editor-bg', '#1e1e1e'),
    foreground: cssColor('--editor-text', '#d4d4d4'),
    accent: cssColor('--accent', '#4f8cff'),
    selection: cssColor('--editor-selection', '#264f78'),
    isDark: document.documentElement.classList.contains('dark'),
  });
}

function readStoredFontSize(): number {
  const value = Number(localStorage.getItem(TERMINAL_FONT_SIZE_KEY));
  return clampTerminalFontSize(value || TERMINAL_FONT_SIZE_DEFAULT);
}

export default function WebTerminal({ active = true, cwd = '', theme, headerActions }: WebTerminalProps) {
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const fitFrameRef = useRef(0);
  const fontSizeRef = useRef(readStoredFontSize());
  const [fontSize, setFontSize] = useState(fontSizeRef.current);
  const [connectionState, setConnectionState] = useState<ConnectionState>('connecting');
  const [connectionKey, setConnectionKey] = useState(0);

  const configureTextarea = useCallback(() => {
    const textarea = hostRef.current?.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea');
    if (!textarea) return;
    textarea.inputMode = 'text';
    textarea.autocapitalize = 'off';
    textarea.autocomplete = 'off';
    textarea.autocorrect = false;
    textarea.spellcheck = false;
    textarea.setAttribute('enterkeyhint', 'enter');
  }, []);

  const focusTerminal = useCallback(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;
    terminal.focus();
    configureTextarea();
    hostRef.current?.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea')?.focus({ preventScroll: true });
  }, [configureTextarea]);

  const fitAndResize = useCallback(() => {
    cancelAnimationFrame(fitFrameRef.current);
    fitFrameRef.current = requestAnimationFrame(() => {
      const terminal = terminalRef.current;
      const fitAddon = fitAddonRef.current;
      const host = hostRef.current;
      if (!terminal || !fitAddon || !host?.isConnected || host.clientWidth < 2 || host.clientHeight < 2) return;
      try { fitAddon.fit(); }
      catch { return; }
      const socket = socketRef.current;
      if (socket?.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'resize', cols: terminal.cols, rows: terminal.rows }));
      }
    });
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let disposed = false;
    let teardown: (() => void) | undefined;
    setConnectionState('connecting');

    const openTerminal = async () => {
      try { await document.fonts.load(TERMINAL_FONT_LOAD); }
      catch { /* the monospace fallback keeps the terminal usable */ }
      if (disposed || !host.isConnected) return;

      const terminal = new Terminal({
        allowTransparency: false,
        convertEol: false,
        cursorBlink: true,
        cursorStyle: 'bar',
        fontFamily: TERMINAL_FONT_FAMILY,
        fontSize: fontSizeRef.current,
        lineHeight: 1.18,
        scrollback: 10_000,
        theme: terminalThemeFromDocument(),
      });
      const fitAddon = new FitAddon();
      terminal.loadAddon(fitAddon);
      terminal.open(host);
      terminalRef.current = terminal;
      fitAddonRef.current = fitAddon;
      configureTextarea();

      const socket = new WebSocket(terminalWebSocketUrl(window.location, cwd));
      socket.binaryType = 'arraybuffer';
      socketRef.current = socket;
      const decoder = new TextDecoder();
      socket.onopen = () => {
        if (disposed) return;
        setConnectionState('connected');
        fitAndResize();
        focusTerminal();
      };
      socket.onmessage = (event) => {
        if (disposed) return;
        if (event.data instanceof ArrayBuffer) terminal.write(decoder.decode(event.data, { stream: true }));
        else if (event.data instanceof Blob) void event.data.arrayBuffer().then((data) => { if (!disposed) terminal.write(decoder.decode(data, { stream: true })); });
        else terminal.write(String(event.data));
      };
      socket.onerror = () => { if (!disposed) setConnectionState('error'); };
      socket.onclose = () => { if (!disposed) setConnectionState('disconnected'); };

      const inputDisposable = terminal.onData((data) => {
        if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ type: 'input', data }));
      });
      const resizeObserver = new ResizeObserver(() => fitAndResize());
      resizeObserver.observe(host);
      fitAndResize();

      teardown = () => {
        cancelAnimationFrame(fitFrameRef.current);
        resizeObserver.disconnect();
        inputDisposable.dispose();
        socket.close(1000, 'terminal view closed');
        terminal.dispose();
        if (socketRef.current === socket) socketRef.current = null;
        if (terminalRef.current === terminal) terminalRef.current = null;
        if (fitAddonRef.current === fitAddon) fitAddonRef.current = null;
      };
    };
    void openTerminal();

    return () => {
      disposed = true;
      teardown?.();
    };
  }, [configureTextarea, connectionKey, cwd, fitAndResize, focusTerminal]);

  useEffect(() => {
    if (!active) return;
    const activeFrame = requestAnimationFrame(() => {
      fitAndResize();
      focusTerminal();
    });
    return () => cancelAnimationFrame(activeFrame);
  }, [active, fitAndResize, focusTerminal]);

  useEffect(() => {
    const themeFrame = requestAnimationFrame(() => {
      const terminal = terminalRef.current;
      if (!terminal) return;
      terminal.options.theme = terminalThemeFromDocument();
      terminal.refresh(0, Math.max(0, terminal.rows - 1));
    });
    return () => cancelAnimationFrame(themeFrame);
  }, [theme]);

  useEffect(() => {
    fontSizeRef.current = fontSize;
    localStorage.setItem(TERMINAL_FONT_SIZE_KEY, String(fontSize));
    const terminal = terminalRef.current;
    if (!terminal) return;
    terminal.options.fontFamily = TERMINAL_FONT_FAMILY;
    terminal.options.fontSize = fontSize;
    fitAndResize();
  }, [fitAndResize, fontSize]);

  const changeFontSize = (delta: number) => setFontSize((value) => clampTerminalFontSize(value + delta));
  const stateLabel = t(`terminal.status.${connectionState}`);

  return <main className={`main-panel terminal-main ${active ? '' : 'terminal-main-hidden'}`} aria-hidden={!active}>
    <header className="chat-header header-no-drawer terminal-header">
      <div className="terminal-heading"><TerminalSquare /><div><h1>{t('terminal.title')}</h1><span>{stateLabel}</span></div></div>
      <div className="terminal-navigation">{headerActions}</div>
    </header>
    <section className="web-terminal-shell" aria-label={t('terminal.title')}>
      <div className="terminal-toolbar" aria-label={t('terminal.title')}>
        <button type="button" className="icon-btn mobile-terminal-keyboard" aria-label={t('terminal.keyboard')} title={t('terminal.keyboard')} onClick={focusTerminal}><Keyboard /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.clear')} title={t('terminal.clear')} onClick={() => terminalRef.current?.clear()}><Eraser /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.fontDecrease')} title={t('terminal.fontDecrease')} disabled={fontSize <= 11} onClick={() => changeFontSize(-1)}><Minus /></button>
        <output className="terminal-font-size" aria-label={t('terminal.fontSize')}>{fontSize}px</output>
        <button type="button" className="icon-btn" aria-label={t('terminal.fontIncrease')} title={t('terminal.fontIncrease')} disabled={fontSize >= 24} onClick={() => changeFontSize(1)}><Plus /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.reconnect')} title={t('terminal.reconnect')} onClick={() => setConnectionKey((value) => value + 1)}><RefreshCw /></button>
      </div>
      <div className="web-terminal-host" ref={hostRef} onPointerDown={focusTerminal} />
    </section>
  </main>;
}
