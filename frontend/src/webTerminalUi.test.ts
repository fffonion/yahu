import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const component = () => readFileSync(new URL('./WebTerminal.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const main = () => readFileSync(new URL('./main.tsx', import.meta.url), 'utf8');
const terminalFontCss = () => readFileSync(new URL('./webTerminalFont.css', import.meta.url), 'utf8');
const packageJson = () => JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8'));

describe('web terminal UI', () => {
  test('desktop rail places Terminal immediately before Settings', () => {
    const source = app();
    const rail = source.slice(source.indexOf('<div className="rail">'), source.indexOf('</div>', source.indexOf('<div className="rail">')));
    expect(rail).toContain('nav-terminal');
    expect(rail.indexOf('nav-terminal')).toBeLessThan(rail.indexOf('nav-settings'));
    expect(rail).toContain("setNavMode('terminal', true)");
  });

  test('mobile header places Terminal immediately before Settings', () => {
    const source = app();
    const start = source.indexOf('return <div className="header-theme-control"');
    const controls = source.slice(start, source.indexOf('desktop-only-theme', start));
    expect(controls).toContain('mobile-header-terminal-btn');
    expect(controls.indexOf('mobile-header-terminal-btn')).toBeLessThan(controls.indexOf('mobile-header-settings-btn'));
    expect(source).toContain('onNavigateToTerminal');
  });

  test('terminal loads xterm lazily and exposes keyboard, resize, clear, reconnect, and font controls', () => {
    const source = app();
    const terminal = component();
    expect(source).toContain("lazy(() => import('./WebTerminal'))");
    expect(source).toContain("mode === 'terminal'");
    expect(terminal).toContain("from '@xterm/xterm'");
    expect(terminal).toContain("from '@xterm/addon-fit'");
    expect(terminal).toContain('ResizeObserver');
    expect(terminal).toContain('terminal.focus()');
    expect(terminal).toContain('mobile-terminal-keyboard');
    expect(terminal).toContain("querySelector<HTMLTextAreaElement>('.xterm-helper-textarea')");
    expect(terminal).toContain("textarea.inputMode = 'text'");
    expect(terminal).toContain("textarea.autocapitalize = 'off'");
    expect(terminal).toContain("textarea.autocomplete = 'off'");
    expect(terminal).toContain("textarea.spellcheck = false");
    for (const key of ['terminal.clear', 'terminal.reconnect', 'terminal.fontDecrease', 'terminal.fontIncrease', 'terminal.keyboard']) expect(terminal).toContain(key);
  });

  test('mobile terminal uses a dedicated input bridge for reliable virtual keyboard entry', () => {
    const terminal = component();
    const styles = css();
    expect(terminal).toContain('mobileInputRef');
    expect(terminal).toContain('className="mobile-terminal-input"');
    expect(terminal).toContain('onInput={handleMobileInput}');
    expect(terminal).toContain('onBeforeInput={handleMobileBeforeInput}');
    expect(terminal).toContain('mobileInputData');
    expect(terminal).toContain('terminal.input(data)');
    expect(terminal).toContain('focusMobileInput');
    expect(terminal).toContain('nativeEvent.data');
    expect(terminal).toContain("deleteContentBackward");
    expect(terminal).toContain('pendingInputRef');
    expect(terminal).toContain('mobileTerminalInput');
    expect(terminal).toContain('disableStdin');
    expect(terminal).toContain('onPointerDown={openMobileKeyboard}');
    expect(terminal).toContain('onTouchStart={openMobileKeyboard}');
    expect(terminal).not.toContain('if (event?.currentTarget instanceof HTMLButtonElement) event.preventDefault();');
    expect(terminal).toContain('onPointerDownCapture={focusTerminalSurface}');
    expect(terminal).toContain('stopPropagation');
    expect(styles).toContain('.mobile-terminal-input');
    expect(styles).toContain('font-size:16px');
  });

  test('mobile terminal exposes one-shot modifiers and special keys', () => {
    const terminal = component();
    const styles = css();
    expect(terminal).toContain('mobile-terminal-special-keys');
    for (const label of ['Esc', 'Ctrl', 'Alt', 'Tab', '↑', '↓', '←', '→']) expect(terminal).toContain(`>${label}<`);
    expect(terminal).toContain('aria-pressed={mobileModifiers.ctrl}');
    expect(terminal).toContain('aria-pressed={mobileModifiers.alt}');
    expect(terminal).toContain("terminalSpecialKeySequence('escape')");
    expect(styles).toContain('.mobile-terminal-special-keys');
  });

  test('keeps the mounted terminal session while other tools are active', () => {
    const source = app();
    const terminal = component();
    const styles = css();
    expect(source).toContain("const [terminalMounted, setTerminalMounted] = useState(initialRoute.mode === 'terminal')");
    expect(source).toContain('{terminalMounted && <Suspense');
    expect(source).toContain("<WebTerminal active={mode === 'terminal'} cwd={terminalCwd}");
    expect(source).not.toContain("if (next === 'terminal') setTerminalCwd('')");
    expect(source).toContain("next === 'terminal' ? { mode: 'terminal', cwd: terminalCwd || undefined }");
    expect(terminal).toContain('active?: boolean');
    expect(terminal).toContain("terminal-main-hidden");
    expect(terminal).toContain('if (!active) return');
    expect(styles).toContain('.terminal-main.terminal-main-hidden{display:none}');
  });

  test('terminal bundles a monospace Nerd Font and loads it before xterm measurement', () => {
    const terminal = component();
    const styles = css();
    expect(main()).not.toContain('@fontsource/source-sans-3');
    expect(packageJson().dependencies['@fontsource/source-sans-3']).toBeUndefined();
    expect(styles).not.toContain('Source Sans');
    expect(terminal).toContain("import './webTerminalFont.css'");
    expect(terminal).toContain("const TERMINAL_FONT_FAMILY = '\"SauceCodePro Nerd Font Mono\", \"SauceCodePro NFM\", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace'");
    expect(terminal).toContain('fontFamily: TERMINAL_FONT_FAMILY');
    expect(terminal).toContain('document.fonts.load(TERMINAL_FONT_LOAD)');
    const fontStyles = terminalFontCss();
    expect(fontStyles).toContain('font-family:"SauceCodePro Nerd Font Mono"');
    expect(fontStyles).toContain('SauceCodeProNerdFontMono-Regular.woff2');
    expect(readFileSync(new URL('./assets/fonts/SauceCodeProNerdFontMono-Regular.woff2', import.meta.url)).subarray(0, 4).toString()).toBe('wOF2');
    expect(terminal).toContain("const themeFrame = requestAnimationFrame(() =>");
    expect(terminal).toContain('return () => cancelAnimationFrame(themeFrame)');
    expect(styles).toContain('.terminal-main');
    expect(styles).toContain('.web-terminal-host');
    expect(styles).toContain('background:var(--editor-bg)');
    expect(styles).toContain('@media (max-width:760px)');
    expect(styles).toContain('.terminal-navigation .header-theme-control .desktop-only-theme{display:inline-flex}');
    expect(styles).toContain('.mobile-header-terminal-btn');
    expect(styles).toContain('.mobile-terminal-keyboard');
  });
});
