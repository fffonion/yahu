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

  test('text preview shows only the highlighted pane, not a duplicate plain editor', () => {
    const source = app();
    const component = source.slice(source.indexOf('function WorkspaceEditorPreview'), source.indexOf('function WorkspaceBrowser'));
    expect(component).toContain('workspace-code-highlight');
    expect(component).not.toContain('className="workspace-editor"');
    expect(component).not.toContain('<textarea');
  });

  test('collapsed right workspace rail exposes two themed buttons', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('workspace-collapsed-actions');
    expect(source).toContain('aria-label="Expand workspace"');
    expect(source).toContain('aria-label="Open workspace page"');
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

  test('mobile drawer remains enabled on the workspace route so the file tree is reachable', () => {
    const source = app();
    expect(source).toContain("if (mode !== 'chat' && mode !== 'cron' && mode !== 'workspace') return;");
    expect(source).toContain("disabled={mode !== 'chat' && mode !== 'cron' && mode !== 'workspace'}");
  });
});
