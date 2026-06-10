import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('draft session lifecycle', () => {
  test('new conversation starts a local draft instead of posting a backend session immediately', () => {
    const source = app();
    expect(source).toContain('const DRAFT_SESSION_ID =');
    expect(source).toContain('startDraftSession={startDraftSession}');
    expect(source).toContain('onClick={() => { props.startDraftSession(); props.closeMobileSidebar(); }}');
    expect(source).not.toContain('onClick={() => props.createSession()');
    expect(source).toContain('if (!sessionId || sessionId === DRAFT_SESSION_ID)');
  });

  test('creating the backend session does not send a title so Hermes can auto-title it', () => {
    const source = app();
    expect(source).toContain('const sessionBody = sessionProvider ? { model: sessionModel, provider: sessionProvider } : { model: sessionModel };');
    expect(source).toContain('body: JSON.stringify(sessionBody)');
    expect(source).not.toContain('title: `WebUI ${new Date().toLocaleString()}`');
    expect(source).not.toContain("title: 'New conversation', model: sessionModel");
  });

  test('first send from a draft preserves optimistic messages and skips the empty initial history reload', () => {
    const source = app();
    expect(source).toContain('skipNextHistoryLoadRef');
    expect(source).toContain('skipNextHistoryLoadRef.current = sessionId');
    expect(source).toContain('setMessages(() => [userMsg, assistantMsg]');
    expect(source).toContain('if (skipNextHistoryLoadRef.current === activeSessionId)');
    expect(source).not.toContain('setMessages([]);\n    setHasOlder(false);\n    setHasNewer(false);\n    return session.id;');
  });

  test('draft sessions are not fetched as backend sessions or auto-replaced by list refresh', () => {
    const source = app();
    expect(source).toContain("if (sessionId === DRAFT_SESSION_ID) return");
    expect(source).toContain("if (loadingMessages && direction !== 'latest') return;");
    expect(source).toContain('if (!activeSessionIdRef.current && list.length) setActiveSessionId(list[0].id)');
    expect(source).toContain('useEffect(() => { activeSessionIdRef.current = activeSessionId; }, [activeSessionId]);');
    const start = source.indexOf('const loadSessions = useCallback');
    const end = source.indexOf('const loadSessionDetail = useCallback', start);
    const loadSessionsBlock = source.slice(start, end);
    expect(loadSessionsBlock).not.toContain('!activeSessionId || activeSessionId === DRAFT_SESSION_ID');
  });

  test('session title refresh happens once after the first successful assistant reply', () => {
    const source = app();
    expect(source).toContain('titleRefreshDoneRef');
    expect(source).toContain('refreshSessionTitleOnce(sessionId)');
    expect(source).toContain('titleRefreshDoneRef.current.add(sessionId)');
    expect(source).not.toContain('await loadSessions(filter);\n      await loadWorkspace(workspacePath);');
  });

  test('new button and filter align in one row without inherited filter margin', () => {
    const styles = css();
    expect(styles).toContain('.session-searchbar{display:grid;grid-template-columns:44px minmax(0,1fr);align-items:center');
    expect(styles).toContain('.filter{height:44px;min-height:44px;margin:0');
  });
});
