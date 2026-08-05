import { useCallback, useEffect, useRef, useState, type CompositionEvent, type FormEvent, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent, type ReactNode } from 'react';
import { Eraser, Keyboard, Minus, Plus, RefreshCw, TerminalSquare } from 'lucide-react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import './webTerminalFont.css';
import { t } from './i18n';
import { applyTerminalModifiers, buildTerminalTheme, clampTerminalFontSize, terminalSpecialKeySequence, terminalWebSocketUrl, TERMINAL_FONT_SIZE_DEFAULT, type TerminalModifierState, type TerminalSpecialKey } from './terminalSupport';

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
  const mobileInputRef = useRef<HTMLTextAreaElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const socketRef = useRef<WebSocket | null>(null);
  const pendingInputRef = useRef('');
  const fitFrameRef = useRef(0);
  const composingRef = useRef(false);
  const mobileModifiersRef = useRef<TerminalModifierState>({ ctrl: false, alt: false });
  const fontSizeRef = useRef(readStoredFontSize());
  const [fontSize, setFontSize] = useState(fontSizeRef.current);
  const [connectionState, setConnectionState] = useState<ConnectionState>('connecting');
  const [connectionKey, setConnectionKey] = useState(0);
  const [mobileModifiers, setMobileModifiers] = useState<TerminalModifierState>(mobileModifiersRef.current);

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

  const focusMobileInput = useCallback(() => {
    const input = mobileInputRef.current;
    if (!input) return;
    input.focus({ preventScroll: true });
    input.setSelectionRange(input.value.length, input.value.length);
  }, []);

  const openMobileKeyboard = useCallback((event?: { preventDefault: () => void; stopPropagation: () => void }) => {
    event?.preventDefault();
    event?.stopPropagation();
    focusMobileInput();
  }, [focusMobileInput]);

  const focusTerminal = useCallback(() => {
    const terminal = terminalRef.current;
    if (!terminal) return;
    configureTextarea();
    terminal.focus();
    if (window.matchMedia('(max-width:760px)').matches) focusMobileInput();
    else terminal.textarea?.focus({ preventScroll: true });
  }, [configureTextarea, focusMobileInput]);

  const focusTerminalSurface = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (window.matchMedia('(max-width:760px)').matches) {
      openMobileKeyboard(event);
      return;
    }
    focusTerminal();
  }, [focusTerminal, openMobileKeyboard]);

  const resetMobileModifiers = useCallback(() => {
    const reset = { ctrl: false, alt: false };
    mobileModifiersRef.current = reset;
    setMobileModifiers(reset);
  }, []);

  const sendSocketInput = useCallback((data: string) => {
    if (!data) return;
    const socket = socketRef.current;
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: 'input', data }));
      return;
    }
    pendingInputRef.current += data;
  }, []);

  const mobileTerminalInput = useCallback((data: string) => {
    sendSocketInput(data);
  }, [sendSocketInput]);

  const sendTerminalInput = useCallback((data: string) => {
    if (!data) return;
    const modifiers = mobileModifiersRef.current;
    data = applyTerminalModifiers(data, modifiers);
    if (window.matchMedia('(max-width:760px)').matches) mobileTerminalInput(data);
    else {
      const terminal = terminalRef.current;
      if (!terminal) pendingInputRef.current += data;
      else terminal.input(data);
    }
    if (modifiers.ctrl || modifiers.alt) resetMobileModifiers();
  }, [mobileTerminalInput, resetMobileModifiers]);

  const finishMobileInput = useCallback((textarea: HTMLTextAreaElement, data: string) => {
    textarea.value = '';
    sendTerminalInput(data);
    requestAnimationFrame(() => {
      if (document.activeElement !== mobileInputRef.current) focusMobileInput();
    });
  }, [focusMobileInput, sendTerminalInput]);

  const mobileInputData = useCallback((textarea: HTMLTextAreaElement, nativeEvent: InputEvent) => {
    if (nativeEvent.inputType === 'insertLineBreak') return '\r';
    if (nativeEvent.inputType === 'deleteContentBackward') return '\x7f';
    if (nativeEvent.inputType === 'deleteContentForward') return '\x1b[3~';
    return nativeEvent.data || textarea.value;
  }, []);

  const handleMobileBeforeInput = useCallback((event: FormEvent<HTMLTextAreaElement>) => {
    if (composingRef.current) return;
    const nativeEvent = event.nativeEvent as InputEvent;
    if (!nativeEvent.cancelable || nativeEvent.inputType === 'insertCompositionText') return;
    const textarea = event.currentTarget;
    const data = mobileInputData(textarea, nativeEvent);
    if (!data) return;
    event.preventDefault();
    finishMobileInput(textarea, data);
  }, [finishMobileInput, mobileInputData]);

  const handleMobileInput = useCallback((event: FormEvent<HTMLTextAreaElement>) => {
    if (composingRef.current) return;
    const textarea = event.currentTarget;
    const data = mobileInputData(textarea, event.nativeEvent as InputEvent);
    finishMobileInput(textarea, data);
  }, [finishMobileInput, mobileInputData]);

  const handleMobileCompositionEnd = useCallback((event: CompositionEvent<HTMLTextAreaElement>) => {
    composingRef.current = false;
    finishMobileInput(event.currentTarget, event.currentTarget.value);
  }, [finishMobileInput]);

  const handleMobileKeyDown = useCallback((event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (event.nativeEvent.isComposing) return;
    const sequences: Partial<Record<string, string>> = {
      Backspace: '\x7f',
      Enter: '\r',
      Escape: terminalSpecialKeySequence('escape'),
      Tab: terminalSpecialKeySequence('tab'),
      ArrowUp: terminalSpecialKeySequence('up'),
      ArrowDown: terminalSpecialKeySequence('down'),
      ArrowRight: terminalSpecialKeySequence('right'),
      ArrowLeft: terminalSpecialKeySequence('left'),
    };
    const data = sequences[event.key];
    if (!data) return;
    event.preventDefault();
    event.currentTarget.value = '';
    sendTerminalInput(data);
  }, [sendTerminalInput]);

  const toggleMobileModifier = useCallback((modifier: keyof TerminalModifierState) => {
    const next = { ...mobileModifiersRef.current, [modifier]: !mobileModifiersRef.current[modifier] };
    mobileModifiersRef.current = next;
    setMobileModifiers(next);
  }, []);

  const sendSpecialKey = useCallback((key: TerminalSpecialKey) => {
    sendTerminalInput(terminalSpecialKeySequence(key));
    focusMobileInput();
  }, [focusMobileInput, sendTerminalInput]);

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
        disableStdin: window.matchMedia('(max-width:760px)').matches,
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
        if (pendingInputRef.current) {
          socket.send(JSON.stringify({ type: 'input', data: pendingInputRef.current }));
          pendingInputRef.current = '';
        }
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

      const inputDisposable = terminal.onData((data) => sendSocketInput(data));
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
  }, [configureTextarea, connectionKey, cwd, fitAndResize, focusTerminal, sendSocketInput]);

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
        <button type="button" className="icon-btn mobile-terminal-keyboard" aria-label={t('terminal.keyboard')} title={t('terminal.keyboard')} onPointerDown={openMobileKeyboard} onTouchStart={openMobileKeyboard} onClick={focusMobileInput}><Keyboard /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.clear')} title={t('terminal.clear')} onClick={() => terminalRef.current?.clear()}><Eraser /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.fontDecrease')} title={t('terminal.fontDecrease')} disabled={fontSize <= 11} onClick={() => changeFontSize(-1)}><Minus /></button>
        <output className="terminal-font-size" aria-label={t('terminal.fontSize')}>{fontSize}px</output>
        <button type="button" className="icon-btn" aria-label={t('terminal.fontIncrease')} title={t('terminal.fontIncrease')} disabled={fontSize >= 24} onClick={() => changeFontSize(1)}><Plus /></button>
        <button type="button" className="icon-btn" aria-label={t('terminal.reconnect')} title={t('terminal.reconnect')} onClick={() => setConnectionKey((value) => value + 1)}><RefreshCw /></button>
      </div>
      <div className="mobile-terminal-special-keys" aria-label={t('terminal.specialKeys')}>
        <button type="button" onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('escape')}>Esc</button>
        <button type="button" className={mobileModifiers.ctrl ? 'active' : ''} aria-pressed={mobileModifiers.ctrl} onPointerDown={focusMobileInput} onClick={() => toggleMobileModifier('ctrl')}>Ctrl</button>
        <button type="button" className={mobileModifiers.alt ? 'active' : ''} aria-pressed={mobileModifiers.alt} onPointerDown={focusMobileInput} onClick={() => toggleMobileModifier('alt')}>Alt</button>
        <button type="button" onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('tab')}>Tab</button>
        <button type="button" aria-label={t('terminal.keyUp')} onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('up')}>↑</button>
        <button type="button" aria-label={t('terminal.keyDown')} onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('down')}>↓</button>
        <button type="button" aria-label={t('terminal.keyLeft')} onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('left')}>←</button>
        <button type="button" aria-label={t('terminal.keyRight')} onPointerDown={focusMobileInput} onClick={() => sendSpecialKey('right')}>→</button>
      </div>
      <div className="web-terminal-host" ref={hostRef} onPointerDownCapture={focusTerminalSurface} />
      <textarea
        ref={mobileInputRef}
        className="mobile-terminal-input"
        aria-label={t('terminal.input')}
        inputMode="text"
        autoCapitalize="off"
        autoComplete="off"
        autoCorrect="off"
        spellCheck={false}
        enterKeyHint="enter"
        rows={1}
        onBeforeInput={handleMobileBeforeInput}
        onInput={handleMobileInput}
        onKeyDown={handleMobileKeyDown}
        onCompositionStart={() => { composingRef.current = true; }}
        onCompositionEnd={handleMobileCompositionEnd}
      />
    </section>
  </main>;
}
