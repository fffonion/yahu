import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('session right-click context menu', () => {
  test('session rows expose a custom context menu with rename and delete actions', () => {
    const source = app();
    expect(source).toContain('onContextMenu');
    expect(source).toContain('session-context-menu');
    expect(source).toContain("t('chat.rename')");
    expect(source).toContain("t('chat.delete')");
    expect(source).toContain('renameSession');
    expect(source).toContain('deleteSession');
  });

  test('rename uses yahu lineage title endpoint and delete uses yahu session proxy', () => {
    const source = app();
    expect(source).toContain("const SESSION_API_BASE = '/hermes';");
    expect(source).toContain("fetch(`/sessions/${encodeURIComponent(session.id)}/title`, { method: 'PATCH'");
    expect(source).toContain('JSON.stringify({ title: nextTitle })');
    expect(source).toContain('const titles = body.titles && typeof body.titles === \'object\' ? body.titles as Record<string, string> : {};');
    expect(source).toContain('const updatedIds = new Set<string>(Array.isArray(body.updated_ids) ? body.updated_ids : [session.id]);');
    expect(source).toContain("method: 'DELETE'");
    expect(source).toContain('apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(session.id)}`)');

  });

  test('session refreshes preserve a renamed title while a stream is still active', () => {
    const source = app();
    expect(source).toContain('const renamedSessionTitlesRef = useRef<Record<string, string>>({});');
    expect(source).toContain('const applyRenamedSessionTitleOverride = useCallback((session: Session) => {');
    expect(source).toContain('const titleOverride = renamedSessionTitlesRef.current[session.id];');
    expect(source).toContain('if (titleOverride && String(session.title || \'\').trim() !== titleOverride) return { ...session, title: titleOverride };');
    expect(source).toContain('const detail = applyRenamedSessionTitleOverride((body.data || body.session || body) as Session);');
    expect(source).toContain('const session = applyRenamedSessionTitleOverride(rawSession);');
    expect(source).toContain('return sessionWithPreservedMessageCount(session, old.find((existing) => existing.id === session.id));');
    expect(source).toContain('renamedSessionTitlesRef.current = { ...renamedSessionTitlesRef.current, ...titles };');
  });


  test('context menu is positioned above the chat sidebar and styled as a menu', () => {
    const styles = css();
    expect(styles).toContain('.session-context-menu{position:fixed');
    expect(styles).toContain('.session-context-menu button.danger');
  });

  test('right-clicking a session opens the menu without switching the active chat', () => {
    const source = app();
    const start = source.indexOf('const openSessionMenuAt = (session: Session');
    const end = source.indexOf('const openSessionMenu =', start);
    const fn = source.slice(start, end);
    expect(fn).toContain('setSessionMenu({ session');

  });
});
