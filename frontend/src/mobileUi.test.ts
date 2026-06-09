import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('mobile WebUI layout and touch affordances', () => {
  test('mobile uses bottom route tabs and moves the list drawer button into page headers', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('mobile-bottom-nav');
    expect(source).toContain('MobileHeaderDrawerButton');
    expect(source).toContain('className="mobile-header-drawer rail-btn"');
    expect(source).toContain('toggleMobileSidebar');
    expect(source).toContain('aria-label="Open list"');
    expect(source).toContain("setNavMode('chat')");
    expect(source).toContain("setNavMode('cron')");
    expect(source).toContain("setNavMode('skills')");
    expect(source).toContain("setNavMode('images', true)");
    expect(source).toContain("setNavMode('memory')");
    expect(styles).toContain('@media (max-width: 760px)');
    expect(styles).toContain('.mobile-bottom-nav');
    expect(styles).toContain('.mobile-header-drawer');
    expect(styles).toContain('left:0');
    expect(styles).toContain('right:0');
    expect(styles).toContain('border-radius:0');
    expect(source).not.toContain('nav-drawer');
    expect(source).not.toContain("setMobileSidebarOpen(true)");
    expect(styles).not.toContain('.mobile-bottom-nav .nav-memory,.mobile-bottom-nav .nav-workspace,.mobile-bottom-nav .nav-settings{display:none!important}');
  });

  test('mobile keeps session cron workspace and skills lists in a hidden 80 percent left drawer', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('mobile-sidebar-open');
    expect(source).toContain('mobile-sidebar-backdrop');
    expect(source).toContain("const hasMobileDrawer = (mode: Mode) => mode === 'chat' || mode === 'cron' || mode === 'workspace' || mode === 'skills';");
    expect(styles).toContain('.sidebar{position:fixed');
    expect(styles).toContain('width:80vw');
    expect(styles).toContain('transform:translateX(-100%)');
    expect(styles).toContain('.app-shell.mobile-sidebar-open .sidebar');
    expect(styles).toContain('transform:translateX(0)');
    expect(styles).toContain('.mobile-sidebar-backdrop');
    expect(styles).toContain('.left-body{display:flex');
    expect(styles).toContain('.theme-card{display:none');
  });

  test('mobile hides the chat workspace side panel and keeps touch scroll containers usable', () => {
    const styles = css();
    expect(styles).toContain('.app-shell{min-width:0');
    expect(styles).toContain('.app-shell{grid-template-columns:minmax(0,1fr)');
    expect(styles).toContain('.workspace{display:none!important');
    expect(styles).toContain('.chat-scroll,.sessions,.cron-sidebar-list,.image-grid-wrap,.file-list');
    expect(styles).toContain('-webkit-overflow-scrolling:touch');
    expect(styles).toContain('touch-action:pan-y');
    expect(styles).toContain('padding-bottom:12px');
    expect(styles).toContain('calc(68px + env(safe-area-inset-bottom, 0px))');
  });

  test('mobile moves theme controls to the top right title bar', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('HeaderThemeControl');
    expect(source).toContain('header-theme-control');
    expect(source).toContain('theme-menu');
    expect(styles).toContain('.header-theme-control');
    expect(styles).toContain('.chat-header .header-theme-control');
    expect(styles).toContain('.theme-card{display:none');
  });

  test('mobile long press opens the session menu as right-click replacement', () => {
    const source = app();
    expect(source).toContain('useLongPressContextMenu');
    expect(source).toContain('onPointerDown');
    expect(source).toContain('onPointerCancel');
    expect(source).toContain('window.setTimeout');
    expect(source).toContain('pointerType === \'mouse\'');
  });

  test('mobile buttons that have icon and copy hide copy while keeping labels for accessibility', () => {
    const styles = css();
    expect(styles).toContain('.btn-label{display:none}');
    expect(styles).toContain('.mobile-icon-only .btn-label');
    expect(styles).toContain('.send-btn,.files-chip,.image-actions button,.modalbar button,.cron-detail-actions button,.settings-content button');
  });

  test('mobile agent replies adapt to the viewport while tool rows keep their icon', () => {
    const styles = css();
    expect(styles).toContain('.msg-row.assistant,.msg-row.system{grid-template-columns:minmax(0,1fr);width:100%;max-width:none}');
    expect(styles).toContain('.msg-row.assistant .avatar,.msg-row.system .avatar{display:none}');
    expect(styles).toContain('.msg-row.assistant .msg-content,.msg-row.system .msg-content{grid-column:1;min-width:0;max-width:100%}');
    expect(styles).toContain('.msg-row.tool{grid-template-columns:32px minmax(0,1fr);width:100%;max-width:100%;align-items:start}');
    expect(styles).toContain('.msg-row.tool .avatar{display:grid;width:32px;height:32px}');
    expect(styles).toContain('.msg-row.tool .msg-content{grid-column:2;min-width:0;max-width:100%}');
    expect(styles).toContain('.msg-row.assistant .msg-body pre,.msg-row.system .msg-body pre,.msg-row.tool .msg-body pre{white-space:pre-wrap;overflow-wrap:anywhere}');
  });

  test('mobile chat rows and tool summaries cannot widen the viewport', () => {
    const styles = css();
    expect(styles).toContain('.chat-scroll{padding:10px 10px 12px}');
    expect(styles).toContain('.main-panel,.chat-header,.chat-scroll,.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow-x:hidden}');
    expect(styles).toContain('.chat-header>div:first-of-type{min-width:0;flex:1 1 auto}');
    expect(styles).toContain('.msg-row{grid-template-columns:minmax(0,1fr);width:100%;min-width:0;max-width:100%;gap:8px}');
    expect(styles).toContain('.msg-row .avatar{display:none}');
    expect(styles).toContain('.msg-row.user .msg-content{grid-column:1;justify-self:end;max-width:92%}');
    expect(styles).toContain('.msg-body pre{white-space:pre-wrap;overflow-wrap:anywhere;overflow-x:hidden}');
    expect(styles).toContain('.msg-body pre code{white-space:inherit;overflow-wrap:anywhere;word-break:break-word}');
    expect(styles).toContain('.tool-summary{grid-template-columns:minmax(0,1fr) 20px;padding:10px 12px;gap:6px}');
    expect(styles).toContain('.tool-title,.tool-subtitle{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}');
    expect(styles).toContain('.session-header-times{display:none}');
  });

  test('switching sessions on mobile still loads the new session while a previous history request is in flight', () => {
    const source = app();
    expect(source).not.toContain('if (sessionId === DRAFT_SESSION_ID || loadingMessages) return;');
    expect(source).toContain("if (loadingMessages && direction !== 'latest') return;");
  });
});
