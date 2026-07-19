import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const server = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');

describe('hash route integration', () => {
  test('App writes route transitions to history and listens for browser traversal', () => {
    const source = app();
    expect(source).toContain("from './hashRoute'");
    expect(source).toContain('pushHashRoute(window.history, window.location.hash, route)');
    expect(source).toContain("window.addEventListener('popstate', applyCurrentHashRoute)");
    expect(source).toContain("window.addEventListener('hashchange', applyCurrentHashRoute)");
    expect(source).not.toContain('window.history.replaceState(null');
    expect(source).not.toContain('window.location.hash = buildHashRoute');
    expect(source).toContain('const initialRoute = getCurrentHashRoute();');
  });

  test('chat and cron deep links select the requested item', () => {
    const source = app();
    expect(source).toContain("route.mode === 'chat'");
    expect(source).toContain('switchActiveSession(route.sessionId)');
    expect(source).toContain("route.mode === 'cron'");
    expect(source).toContain('setCronEditingId(route.jobId)');
    expect(source).toContain('beginCronEdit(selectedJob)');
  });

  test('image deep links fetch one image entry and open the modal directly', () => {
    const source = app();
    expect(source).toContain('initialImageFilename');
    expect(source).toContain('fetch(`/image-api/images/${enc(initialImageFilename)}`');
    expect(source).toContain('setModal(entry);');
    expect(server()).toContain('.route("/image-api/images/{filename}", get(image_entry).delete(delete_image))');
  });

  test('workspace deep links expand folders and open files', () => {
    const source = app();
    expect(source).toContain('openWorkspaceRouteTarget');
    expect(source).toContain("workspaceKind: 'file'");
    expect(source).toContain("workspaceKind: 'folder'");
    expect(source).toContain('setExpandedWorkspacePaths((old) => new Set([...Array.from(old), ...parents, targetPath]));');
    expect(source).toContain('await openWorkspacePathFile(targetPath);');
    expect(source).toContain('isWorkspaceTextFile(entry.name)');
  });

  test('nav clicks and item opens write canonical hash routes', () => {
    const source = app();
    expect(source).toContain('writeHashRoute(route);');
    expect(source).toContain("props.writeHashRoute({ mode: 'chat', sessionId: id });");
    expect(source).toContain("buildHashRoute({ mode: 'cron', jobId: jobId(j) })");
    expect(source).toContain("writeHashRoute({ mode: 'images', imageFilename: item.filename })");
    expect(source).toContain("writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path })");
  });

  test('hash landing auto-collapses pages without a left list', () => {
    const source = app();
    expect(source).toContain("setSidebarCollapsed(route.mode === 'images' || route.mode === 'memory' || route.mode === 'insights' || route.mode === 'settings')");
    expect(source).toContain("setSidebarCollapsed(collapse || next === 'memory' || next === 'insights' || next === 'settings')");
  });
});
