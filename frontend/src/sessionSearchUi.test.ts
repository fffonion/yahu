import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('session search and composer session model UI', () => {
  test('left search uses backend results instead of client-side title filtering', () => {
    const app = source();
    expect(app).toContain("fetch(`/sessions/search?");
    expect(app).toContain('searchVersionRef');
    expect(app).not.toContain("`${s.title || ''} ${s.preview || ''}`");
  });

  test('new conversation and cron filter are icon buttons beside the search field', () => {
    const app = source();
    const css = styles();
    expect(app).toContain('className="session-searchbar"');
    expect(app).toContain('aria-label={t(\'chat.new\')}');
    expect(app).toContain('aria-pressed={props.hideCronSessions}');
    expect(app).toContain('setHideCronSessions={(value: boolean) => setHideCronSessions(value)}');
    expect(app).toContain('SlidersHorizontal');
    expect(app).not.toContain('FileText, Filter, Folder');
    expect(app).not.toContain('<Filter />');
    expect(app).not.toContain('<span>New conversation</span>');
    expect(css).toContain('.session-searchbar{display:grid;grid-template-columns:44px minmax(0,1fr) 44px');
    expect(css).toContain('.filter{height:44px');
    expect(css).toContain('.session-filter-btn{width:44px;height:44px');
  });

  test('composer model comes from selected session details, not a global Hermes fallback', () => {
    const app = source();
    expect(app).toContain('activeSessionDetail');
    expect(app).toContain('loadSessionDetail(activeSessionId)');
    expect(app).toContain('const sessionModel = sessionModelOverride?.model || realModelOrEmpty(active?.model) || realModelOrEmpty(props.activeSessionDetail?.model) || realModelOrEmpty(props.model) || props.models[0]?.id ||');
    expect(app).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
    expect(app).toContain('const exactCurrentOption = currentModel ? findModelOption(props.models, currentModel, sessionProvider) : undefined;');
    expect(app).toContain('const currentOption = currentModel && !exactCurrentOption ? currentModelDisplayOption(currentModel, props.models, sessionProvider) : undefined;');
    expect(app).not.toContain('props.runtimeProvider');
  });

  test('mock stream UI and send branch are removed', () => {
    const app = source();
    expect(app).not.toContain('useMockStream');
    expect(app).not.toContain('mock-toggle');
    expect(app).not.toContain('/mock-stream');
    expect(app).not.toContain('Mock stream');
  });

  test('session rows keep missing titles as a dash instead of promoting preview text', () => {
    const app = source();
    expect(app).toContain('sessionDisplayTitle(session)');
    expect(app).not.toContain('<span className="session-title">{session.title || session.preview || session.id}</span>');
    expect(app).not.toContain("session.title || '—'");
  });

  test('active session sidebar preview is only changed by frontend during live streaming', () => {
    const app = source();
    expect(app).toContain("import { latestSessionPreviewFromMessages } from './sessionPreview';");
    expect(app).toContain('if (streamingSessionId !== activeSessionId) return;');
    expect(app).toContain('const activePreview = latestSessionPreviewFromMessages(messages);');
    expect(app).toContain('setSessions((old) => old.map((session) => session.id === activeSessionId && session.preview !== activePreview ? { ...session, preview: activePreview } : session));');
    expect(app).toContain('setActiveSessionDetail((old) => old?.id === activeSessionId && old.preview !== activePreview ? { ...old, preview: activePreview } : old);');
    expect(app).toContain('setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, content: text, pending: true, timestamp: Date.now() / 1000 } : m));');
  });

  test('opened session header uses stitched history totals from message and minimap endpoints', () => {
    const app = source();
    expect(app).toContain('const updateSessionMessageCount = useCallback((sessionId: string, total: unknown) => {');
    expect(app).toContain('sessionWithPreservedMessageCount(detail, old)');
    expect(app).toContain('sessionWithPreservedMessageCount(session, old.find((existing) => existing.id === session.id))');
    expect(app).toContain('updateSessionMessageCount(sessionId, page.total);');
    expect(app).toContain('updateSessionMessageCount(sessionId, body.total);');
    expect(app).toContain("<span>{props.messages.length || 0} loaded · {active?.message_count || 0} total</span>");
  });

  test('opened session header shows start and latest message times on the right', () => {
    const app = source();
    const css = styles();
    expect(app).toContain("import { formatChatMessageTime, sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';");
    expect(app).toContain('const headerTimes = sessionHeaderTimes(active, props.messages);');
    expect(app).toContain('className="session-header-times"');
    expect(css).toContain('.session-header-times{');
  });
});
