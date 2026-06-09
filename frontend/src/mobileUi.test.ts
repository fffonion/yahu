import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('mobile WebUI layout and touch affordances', () => {
  test('mobile uses a full-width bottom toolbar with chat cron images memory and a list drawer button', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('mobile-bottom-nav');
    expect(source).toContain('toggleMobileSidebar');
    expect(source).toContain('aria-label="Open list"');
    expect(source).toContain("setNavMode('chat')");
    expect(source).toContain("setNavMode('cron')");
    expect(source).toContain("setNavMode('images', true)");
    expect(source).toContain("setNavMode('memory')");
    expect(styles).toContain('@media (max-width: 760px)');
    expect(styles).toContain('.mobile-bottom-nav');
    expect(styles).toContain('left:0');
    expect(styles).toContain('right:0');
    expect(styles).toContain('border-radius:0');
    expect(styles).not.toContain('.mobile-bottom-nav .nav-memory,.mobile-bottom-nav .nav-workspace,.mobile-bottom-nav .nav-settings{display:none!important}');
  });

  test('mobile keeps session and cron lists in a hidden 80 percent left drawer', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('mobile-sidebar-open');
    expect(source).toContain('mobile-sidebar-backdrop');
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
    expect(styles).toContain('padding-bottom:calc(88px + env(safe-area-inset-bottom, 0px))');
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
});
