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
    expect(source).toContain("aria-label={t('nav.openList')}");
    expect(source).toContain("setNavMode('chat')");
    expect(source).toContain("setNavMode('cron')");
    expect(source).toContain("setNavMode('skills')");
    expect(source).toContain("setNavMode('insights', true)");
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
    expect(styles).toContain('calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom, 0px))');
  });

  test('mobile memory editor keeps both textareas internally scrollable', () => {
    const styles = css();
    expect(styles).toContain('.memory-main .admin-content{min-height:0;overflow:hidden;padding:12px 12px calc(96px + env(safe-area-inset-bottom,0px))}');
    expect(styles).toContain('.memory-grid{min-height:0;height:100%;grid-template-columns:1fr;grid-template-rows:minmax(0,1fr) minmax(0,1fr) auto;overflow:hidden}');
    expect(styles).toContain('.memory-grid textarea{min-height:0;overflow:auto;touch-action:pan-y;-webkit-overflow-scrolling:touch;overscroll-behavior:contain}');
  });

  test('mobile settings uses the content panel as the scroll container', () => {
    const styles = css();
    expect(styles).toContain('.settings-main{overflow:hidden}');
    expect(styles).toContain('.settings-content{padding:16px 16px calc(96px + env(safe-area-inset-bottom,0px));min-height:0;overflow:auto;touch-action:pan-y;-webkit-overflow-scrolling:touch;overscroll-behavior:contain}');
  });

  test('mobile composer bottom padding matches the rendered bottom nav height without an extra gap', () => {
    const styles = css();
    const navHeight = Number(styles.match(/--mobile-bottom-nav-height:(\d+)px/)?.[1]);
    const railHeight = Number(styles.match(/\.mobile-bottom-nav \.rail-btn\{width:48px;height:(\d+)px\}/)?.[1]);
    const padding = styles.match(/\.mobile-bottom-nav\{[^}]*padding:(\d+)px [^}]+ calc\((\d+)px \+ env\(safe-area-inset-bottom,0px\)\)/);

    expect(styles).toContain('.composer-wrap{padding:10px 10px calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom, 0px))}');
    expect(Number.isFinite(navHeight)).toBe(true);
    expect(Number.isFinite(railHeight)).toBe(true);
    expect(padding).not.toBeNull();
    expect(navHeight).toBe(railHeight + Number(padding?.[1]) + Number(padding?.[2]) + 1);
  });

  test('mobile composer expanded mode has no inner frame while keeping the previous compact control row', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap:not(.composer-compact){padding:0 0 calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom,0px));background:var(--surface);box-shadow:0 -8px 22px rgba(0,0,0,.08)}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-box{width:100%;border:0;border-radius:0;box-shadow:none;background:transparent}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-box textarea{display:block;width:100%;height:48px;min-height:48px;max-height:20dvh;padding:12px 14px;border:0;border-radius:0;box-shadow:none;background:var(--surface);resize:none;overflow-y:hidden}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-box textarea:focus{border:0;box-shadow:none}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer{position:relative;display:flex;gap:0;width:100%;padding:24px 10px 8px;border-top:0;background:transparent;overflow:visible;flex-wrap:nowrap}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .dropdown-control.wide{min-width:64px;margin-left:8px}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .dropdown-control.wide+.dropdown-control{margin-left:4px}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .reasoning-view-toggle{margin-left:8px}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .tool-call-view-toggle{margin-left:4px}');
    expect(styles).toContain('.composer-footer .dropdown-trigger{border-radius:var(--radius-md)}');
    expect(styles).not.toContain('.composer-wrap:not(.composer-compact) .composer-footer{display:grid;grid-template-columns:repeat(4,minmax(0,1fr))');
    expect(styles).not.toContain('.composer-wrap:not(.composer-compact) .composer-footer .attach-btn,.composer-wrap:not(.composer-compact) .composer-footer .send-btn,.composer-wrap:not(.composer-compact) .composer-footer .dropdown-control{width:100%;min-width:0;height:44px}');
  });

  test('expanded chat composer textarea uses one row so single-line input does not grow', () => {
    const source = app();
    expect(source).toContain('<textarea ref={textareaRef} rows={1}');
    expect(source).toContain('const minHeight = 48;');
    expect(source).toContain('const contentHeight = textarea.value.trim() ? textarea.scrollHeight : minHeight;');
  });

  test('composer collapses while reading history on desktop and mobile, then restores controls on input focus', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const [composerCompact, setComposerCompact] = useState(false);');
    expect(source).toContain("className={`composer-wrap ${props.composerCompact ? 'composer-compact' : ''}`}");
    expect(source).not.toContain("window.matchMedia('(max-width: 760px)').matches");
    expect(source).toContain('const collapseComposerForHistory = () => {');
    expect(source).toContain('activeElement.blur();');
    expect(source).toContain('onPointerDown={collapseComposerForHistory}');
    expect(source).toContain('onTouchStart={collapseComposerForHistory}');
    expect(source).toContain('onWheel={onWheel}');
    expect(source).toContain('const onWheel = (e: React.WheelEvent<HTMLElement>) => {');
    expect(source).toContain('collapseComposerForHistory();');
    expect(source).toContain('!props.composerRef.current?.contains(document.activeElement)');
    expect(source).toContain('props.setComposerCompact(true)');
    expect(source).toContain('onFocus={() => props.setComposerCompact(false)}');
    expect(styles).toContain('.chat-header-actions .context-window-meter{margin-left:0');
    expect(styles).toContain('.composer-footer .send-btn{margin-left:0;flex:0 0 auto}');
    expect(styles).toContain('.composer-wrap.composer-compact .attachments{display:none}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .attach-btn,.composer-wrap.composer-compact .composer-footer .dropdown-control,.composer-wrap.composer-compact .composer-footer .reasoning-view-toggle,.composer-wrap.composer-compact .composer-footer .tool-call-view-toggle{display:none}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-box textarea{height:48px;min-height:48px;max-height:48px;overflow:hidden;padding:10px 56px 10px 14px}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer{position:absolute;right:0;bottom:12px;width:auto;border-top:0;background:transparent;padding:0}');
    expect(styles).toContain('.composer-wrap.composer-compact{padding:8px 18px}');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-box{min-height:64px}');
  });

  test('compact composer send button keeps the same right inset as expanded composer', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer{position:relative;display:flex;gap:0;width:100%;padding:24px 10px 8px;border-top:0;background:transparent');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer{position:absolute;right:0;bottom:12px;');
    expect(styles).toContain('.chat-header-actions .context-window-meter{margin-left:0');
    expect(styles).toContain('.composer-footer .send-btn{margin-left:0;flex:0 0 auto}');
  });

  test('mobile bottom nav paints above the composer reserved area but below open dropdown menus', () => {
    const styles = css();
    const composerZ = Number(styles.match(/\.composer-wrap\{[^}]*z-index:(\d+)/)?.[1]);
    const navZ = Number(styles.match(/\.mobile-bottom-nav\{[^}]*z-index:(\d+)/)?.[1]);
    const menuZ = Number(styles.match(/\.dropdown-control\.open \.dropdown-menu\{z-index:(\d+)\}/)?.[1]);

    expect(Number.isFinite(composerZ)).toBe(true);
    expect(Number.isFinite(navZ)).toBe(true);
    expect(Number.isFinite(menuZ)).toBe(true);
    expect(navZ).toBeGreaterThan(composerZ);
    expect(menuZ).toBeGreaterThan(navZ);
  });

  test('mobile bottom nav active item uses color only without a pill background or frame', () => {
    const styles = css();
    expect(styles).toContain('.mobile-bottom-nav .rail-btn.active,.mobile-bottom-nav .rail-btn.active:hover{background:transparent;box-shadow:none;color:var(--rail-accent);border-color:transparent}');
    expect(styles).toContain('.mobile-bottom-nav .rail-btn.nav-skills.active{background:transparent;color:var(--rail-accent);box-shadow:none}');
  });

  test('mobile moves theme controls to the top right title bar', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('HeaderThemeControl');
    expect(source).toContain('header-theme-control');
    expect(source).toContain('theme-menu');
    expect(source).toContain('role="menuitemradio"');
    expect(source).toContain('aria-checked={theme === item.id}');
    expect(source).not.toContain('<label><span>Theme</span><select value={theme} onChange={(event) => setTheme(event.target.value as Theme)}');
    expect(styles).toContain('.header-theme-control');
    expect(styles).toContain('.chat-header .header-theme-control');
    expect(styles).toContain('.theme-menu button:hover,.theme-menu button.active');
    expect(styles).toContain('.theme-card{display:none');
  });

  test('mobile title bars stay compact and no-drawer pages align titles with drawer pages', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('chat-header header-no-drawer');
    expect(source).toContain('image-toolbar header-no-drawer');
    expect(styles).toContain('.chat-header,.image-toolbar{min-height:44px;padding:4px 9px;display:flex');
    expect(styles).toContain('.chat-header h1,.image-toolbar h1{font-size:16px;line-height:1.15;transform:translateY(1px)}');
    expect(styles).toContain('.chat-header span,.image-toolbar span{font-size:11px;line-height:1.1}');
    expect(styles).toContain('.image-actions>button{width:30px;height:30px}');
    expect(styles).toContain('.mobile-header-drawer{display:inline-grid;width:30px;height:30px;min-width:30px;border-radius:10px;flex:0 0 30px}');
    expect(styles).toContain('.chat-header .header-theme-control>button,.image-toolbar .header-theme-control>button{height:30px;min-width:30px');
    expect(styles).toContain('@media (max-width:760px){.chat-header-actions .context-window-meter{width:116px;min-width:0;max-width:116px;flex:0 1 116px}.chat-header-actions .context-window-track{min-width:30px}');
    expect(styles).toContain('.chat-header-actions .header-theme-control{margin-left:0}');
    expect(styles).toContain('.chat-header.header-no-drawer>div:first-of-type,.image-toolbar.header-no-drawer>div:first-of-type{margin-left:36px}');
  });

  test('mobile insights keeps charts readable and avoids horizontal overflow', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function InsightsMain');
    expect(source).toContain('UsageAreaChart');
    expect(source).toContain('linearGradient');
    expect(styles).toContain('.insights-main{grid-column:2 / -1;--chart-0:#14b8a6');
    expect(styles).toContain('.usage-area{opacity:.78;transform-origin:center bottom;animation:chart-fill .55s ease both}');
    expect(styles).toContain('.usage-total-line{fill:none;stroke:var(--accent);stroke-width:1.06');
    expect(styles).toContain('.usage-line{fill:none;stroke-width:1');
    expect(styles).toContain('.usage-share-bar{height:9px;border:1px solid var(--border);border-radius:999px;overflow:hidden;display:flex;');
    expect(styles).toContain('@media (max-width:760px){.mobile-bottom-nav .rail-btn.nav-insights.active');
    expect(styles).toContain('.insights-content{padding:10px 10px calc(86px + env(safe-area-inset-bottom,0px));gap:10px');
    expect(styles).toContain('.usage-chart{min-height:230px;padding-bottom:46px;overflow:visible}');
    expect(styles).toContain('.usage-line{stroke-width:.94}');
    expect(styles).toContain('.chart-y-axis{left:2px;bottom:46px;width:28px;font-size:10px}.chart-y-axis span{right:2px}');
    expect(styles).toContain('.insights-cards{grid-template-columns:repeat(2,minmax(0,1fr));gap:8px}');
  });

  test('mobile long press opens the session menu as right-click replacement', () => {
    const source = app();
    expect(source).toContain('useLongPressContextMenu');
    expect(source).toContain('onPointerDown');
    expect(source).toContain('onPointerCancel');
    expect(source).toContain('window.setTimeout');
    expect(source).toContain('pointerType === \'mouse\'');
  });

  test('mobile session rows avoid hover-only pin reveal so the first tap activates the row', () => {
    const styles = css();
    expect(styles).toContain('@media (max-width:760px){.session-item .pin-hit{opacity:1;pointer-events:auto;transition:none}');
  });

  test('mobile buttons that have icon and copy hide copy while keeping labels for accessibility', () => {
    const styles = css();
    expect(styles).toContain('.btn-label{display:none}');
    expect(styles).toContain('.mobile-icon-only .btn-label');
    expect(styles).toContain('.send-btn,.files-chip,.image-actions button,.modalbar button,.settings-content button');
  });

  test('mobile agent replies adapt to the viewport while tool icons live inside full-width cards', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('<span className="tool-inline-icon">{getToolIcon(toolName)}</span>');
    expect(styles).toContain('.msg-row.assistant,.msg-row.system{grid-template-columns:minmax(0,1fr);width:100%;max-width:none}');
    expect(styles).toContain('.msg-row.assistant .avatar,.msg-row.system .avatar{display:none}');
    expect(styles).toContain('.msg-row.assistant .msg-content,.msg-row.system .msg-content{grid-column:1;min-width:0;max-width:100%}');
    expect(styles).toContain('.tool-inline-icon{display:none;color:var(--accent);place-items:center}');
    expect(styles).toContain('.msg-row.tool{grid-template-columns:minmax(0,1fr);width:100%;max-width:100%;align-items:start}');
    expect(styles).toContain('.msg-row.tool .avatar{display:none}');
    expect(styles).toContain('.msg-row.tool .msg-content{grid-column:1;min-width:0;max-width:100%}');
    expect(styles).toContain('.msg-row.tool .tool-inline-icon{display:grid;grid-column:1;grid-row:1 / 3}');
    expect(styles).toContain('.msg-row.assistant .msg-body pre,.msg-row.system .msg-body pre,.msg-row.tool .msg-body pre{white-space:pre-wrap;overflow-wrap:anywhere}');
  });

  test('mobile chat rows and tool summaries cannot widen the viewport', () => {
    const styles = css();
    expect(styles).toContain('.chat-scroll{padding:10px 10px 12px}');
    expect(styles).toContain('.main-panel,.chat-header,.chat-scroll{min-width:0;width:100%;max-width:100vw;overflow-x:hidden}');
    expect(styles).toContain('.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow:visible;position:relative;z-index:160;padding:10px 10px calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom,0px))}');
    expect(styles).toContain('.chat-header>div:first-of-type{min-width:0;flex:1 1 auto}');
    expect(styles).toContain('.msg-row{grid-template-columns:minmax(0,1fr);width:100%;min-width:0;max-width:100%;gap:8px}');
    expect(styles).toContain('.msg-row .avatar{display:none}');
    expect(styles).toContain('.msg-row.user .msg-content{grid-column:1;justify-self:end;max-width:92%}');
    expect(styles).toContain('.msg-body pre{white-space:pre-wrap;overflow-wrap:anywhere;overflow-x:hidden}');
    expect(styles).toContain('.msg-body pre code{white-space:inherit;overflow-wrap:anywhere;word-break:break-word}');
    expect(styles).toContain('.tool-summary{grid-template-columns:24px minmax(0,1fr) 20px;padding:10px 12px;gap:6px}');
    expect(styles).toContain('.tool-title,.tool-subtitle{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}');
    expect(styles).toContain('.session-header-times{display:none}');
  });

  test('switching sessions on mobile still loads the new session while a previous history request is in flight', () => {
    const source = app();
    expect(source).not.toContain('if (sessionId === DRAFT_SESSION_ID || loadingMessages) return;');
    expect(source).toContain("if (loadingMessagesRef.current && direction !== 'latest') return;");
  });

  test('tapping the active mobile history row closes the drawer without resetting the current chat', () => {
    const source = app();
    expect(source).toContain("if (id === props.activeSessionId) {");
    expect(source).toContain("props.writeHashRoute({ mode: 'chat', sessionId: id });");
    expect(source).toContain('props.closeMobileSidebar();\n      return;');
  });
});
