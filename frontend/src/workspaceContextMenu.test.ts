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
    expect(source).toContain("t('workspace.expand')");

    expect(source).toContain("t('workspace.collapse')");
  });

  test('workspace rows expose a right-click view/edit/rename/delete menu', () => {
    const source = app();
    expect(source).toContain('workspace-context-menu');
    expect(source).toContain('openWorkspaceMenu');
    expect(source).toContain("t('workspace.viewItem')");
    expect(source).toContain("t('workspace.editItemPage')");
    expect(source).toContain('editWorkspaceEntryPage');
    expect(source).toContain('workspaceEdit');
    expect(source).toContain("t('workspace.renameItem')");
    expect(source).toContain("t('workspace.deleteItem')");
    expect(source).toContain("method: 'PATCH'");
    expect(source).toContain("method: 'DELETE'");
  });

  test('workspace folders open a terminal whose cwd survives canonical routing', () => {
    const source = app();
    expect(source).toContain('openWorkspaceTerminal');
    expect(source).toContain("workspaceMenu.entry.kind === 'dir'");
    expect(source).toContain("t('workspace.openInTerminal')");
    expect(source).toContain("writeHashRoute({ mode: 'terminal', cwd: entry.path })");
    expect(source).toContain("<WebTerminal active={mode === 'terminal'} cwd={terminalCwd}");
  });

  test('workspace side panel collapsed layout keeps a narrow expand rail', () => {
    const styles = css();
    expect(styles).toContain('.app-shell.workspace-collapsed');
    expect(styles).toContain('.workspace.workspace-collapsed');
    expect(styles).toContain('.workspace-expand');
  });

  test('workspace rows use extension and folder-aware icon descriptors', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("import { WorkspaceEntryIcon } from './workspaceIcons';");
    expect(source).toContain('<WorkspaceEntryIcon entry={entry} expanded={expanded} />');
    expect(styles).toContain('.workspace-entry-icon');
    expect(styles).toContain('.icon-tone-typescript');
    expect(styles).toContain('.icon-tone-folder-tests');
  });

  test('workspace row heights stay fixed when folder trees overflow', () => {
    const styles = css();
    expect(styles).toContain('.file-row{display:grid;grid-template-columns:18px 18px minmax(0,1fr) auto 28px;gap:8px;align-items:center;padding:8px;border-radius:12px;cursor:pointer;flex:0 0 auto}');
  });

  test('workspace context menus close on outside pointer down', () => {
    const source = app();
    expect(source).toContain("window.addEventListener('pointerdown', onPointerDown, true)");
    expect(source).toContain("target?.closest('.session-context-menu,.skill-context-menu,.skill-file-context-menu,.workspace-context-menu')");
  });

  test('backend exposes workspace item rename and delete routes', () => {
    const source = server();
    expect(source).toContain('.route("/workspace/item", patch(workspace_rename).delete(workspace_delete))');
    expect(source).toContain('struct WorkspaceRenamePayload');
    expect(source).toContain('workspace_destination_path');
  });
});
