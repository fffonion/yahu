import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('workspace page file tree layout', () => {
  test('workspace mode reuses the left sidebar for an expandable file tree', () => {
    const source = app();
    expect(source).toContain('WorkspaceSidebar');
    expect(source).toContain("mode === 'workspace' ? <WorkspaceSidebar");
    expect(source).toContain('workspace-tree');
    expect(source).toContain('expandedWorkspacePaths');
    expect(source).toContain('toggleWorkspaceFolder');
    expect(source).not.toContain("e.kind === 'dir') loadWorkspace(entry.path)");
  });

  test('workspace main pane is dedicated to editor or preview', () => {
    const source = app();
    expect(source).toContain('WorkspaceEditorPreview');
    expect(source).toContain('workspace-editor-preview');
    expect(source).toContain('workspace-code-highlight');
    expect(source).toContain('highlightWorkspaceText');
    expect(source).toContain("highlightWorkspaceText(preview.content || '', preview.path)");
    expect(source).toContain('preview.kind === \'image\'');
  });

  test('text preview shows highlighted pane and an edit-mode textarea', () => {
    const source = app();
    const component = source.slice(source.indexOf('function WorkspaceEditorPreview'), source.indexOf('function WorkspaceBrowser'));
    expect(component).toContain('workspace-code-highlight');
    expect(component).toContain('workspace-editor-textarea');
    expect(component).toContain('editMode');
  });

  test('collapsed right workspace rail only exposes the expand control', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('workspace-collapsed-actions');
    expect(source).toContain('aria-label="Expand workspace"');
    expect(source).not.toContain('aria-label="Open workspace page"');
    expect(styles).toContain('.workspace-collapsed-actions');
    expect(styles).toContain('.workspace-rail-btn');
  });

  test('expanded session workspace uses the header X as its only collapse control', () => {
    const source = app();
    const styles = css();
    const browser = source.slice(source.indexOf('function WorkspaceBrowser'), source.indexOf('function AdminMain'));
    expect(browser).toContain("aria-label={compact ? t('workspace.collapse') : undefined}");
    expect(browser).toContain('onClick={() => compact ? setCollapsed(true) : setPreview');
    expect(browser).not.toContain('workspace-collapse-btn');
    expect(styles).not.toContain('workspace-collapse-btn');
  });

  test('chat right workspace reserves scrollable middle row and visible bottom preview', () => {
    const source = app();
    const styles = css();
    const browser = source.slice(source.indexOf('function WorkspaceBrowser'), source.indexOf('function AdminMain'));
    expect(browser).toContain("{preview.kind !== 'none' && <div className=\"preview\"");
    expect(styles).toContain('.workspace{border-left-width:1px;border-top-width:0;border-bottom-width:0;border-right-width:0;height:100vh;display:grid;grid-template-rows:auto minmax(0,1fr) auto;overflow:hidden}');
    expect(styles).toContain('.workspace-tree.file-list{min-height:0}');
  });

  test('chat right workspace preview can jump to full workspace preview', () => {
    const source = app();
    const browser = source.slice(source.indexOf('function WorkspaceBrowser'), source.indexOf('function AdminMain'));
    expect(source).toContain('Maximize2');
    expect(source).toContain("'workspace.openFullPreview'");
    expect(browser).toContain('aria-label={t(\'workspace.openFullPreview\')}');
    expect(browser).toContain("window.location.hash = buildHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: preview.path });");
    expect(browser).toContain('aria-label="Close preview"');
  });

  test('chat right workspace previews files in place without changing to workspace route', () => {
    const source = app();
    const browser = source.slice(source.indexOf('function WorkspaceBrowser'), source.indexOf('function AdminMain'));
    expect(source).toContain('options?: { edit?: boolean; route?: boolean }');
    expect(browser).toContain("openWorkspaceEntry(entry, compact ? { route: false } : undefined)");
    expect(source).toContain("if (options?.route !== false) writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });");
  });

  test('mobile drawer remains enabled on the workspace route so the file tree is reachable', () => {
    const source = app();
    expect(source).toContain("const hasMobileDrawer = (mode: Mode) => mode === 'chat' || mode === 'cron' || mode === 'workspace' || mode === 'skills';");
    expect(source).toContain('MobileHeaderDrawerButton');
  });
});
