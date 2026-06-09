import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const server = () => ['mod.rs', 'workspace.rs']
  .map((file) => readFileSync(new URL(`../../src/backend/${file}`, import.meta.url), 'utf8'))
  .join('\n');

describe('workspace collapse and context menu', () => {
  test('right workspace starts collapsed and can be toggled open', () => {
    const source = app();
    expect(source).toContain('workspaceCollapsed, setWorkspaceCollapsed');
    expect(source).toContain('useState(true)');
    expect(source).toContain('workspace-collapsed');
    expect(source).toContain('Expand workspace');
    expect(source).toContain("t('workspace.collapse')");
  });

  test('workspace rows expose a right-click rename/delete menu', () => {
    const source = app();
    expect(source).toContain('WorkspaceContextMenu');
    expect(source).toContain('openWorkspaceMenu');
    expect(source).toContain('workspace-context-menu');
    expect(source).toContain("t('workspace.renameItem')");
    expect(source).toContain("t('workspace.deleteItem')");
    expect(source).toContain("method: 'PATCH'");
    expect(source).toContain("method: 'DELETE'");
  });

  test('workspace side panel collapsed layout keeps a narrow expand rail', () => {
    const styles = css();
    expect(styles).toContain('.app-shell.workspace-collapsed');
    expect(styles).toContain('.workspace.workspace-collapsed');
    expect(styles).toContain('.workspace-expand');
  });

  test('backend exposes workspace item rename and delete routes', () => {
    const source = server();
    expect(source).toContain('.route("/workspace/item", patch(workspace_rename).delete(workspace_delete))');
    expect(source).toContain('struct WorkspaceRenamePayload');
    expect(source).toContain('workspace_destination_path');
  });
});
