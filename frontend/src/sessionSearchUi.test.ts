import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => [readFileSync(new URL('./App.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./chatMessage.ts', import.meta.url), 'utf8')].join('\n');
const styles = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const translations = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');

describe('session search and composer session model UI', () => {
  test('left search uses backend results instead of client-side title filtering', () => {
    const app = source();
    expect(app).toContain("fetch(`/sessions/search?");
    expect(app).toContain("const params = new URLSearchParams({ limit: '80', _: String(Date.now()) });");
    expect(app).toContain("cache: 'no-store'");
    expect(app).toContain('searchVersionRef');
  });

  test('session source filter is sent to the backend before pagination', () => {
    const app = source();
    expect(app).toContain("if (hideCronSessions) params.set('hide_cron_cli', 'true');");
    expect(app).toContain('splitSidebarSessions(sessions, pinnedIds)');
  });

  test('pinned session ids are sent to the backend so they survive the recent window', () => {
    const app = source();
    expect(app).toContain("if (pinnedIds.size) params.set('pinned_ids', Array.from(pinnedIds).join(','));");
    expect(app).toContain('[filter, hideCronSessions, pinnedIds, headers');
  });

  test('canonical redirects migrate a pinned row to the canonical id without dropping selection', () => {
    const app = source();
    expect(app).toContain("setPinnedIds((old) => old.has(sessionId) ? replacePinnedSessionId(old, sessionId, canonicalId) : old);");
    expect(app).toContain("if (old.some((session) => session.id === canonicalId)) return old.filter((session) => session.id !== sessionId);");
    expect(app).toContain("return old.map((session) => session.id === sessionId ? { ...session, id: canonicalId } : session);");
  });

  test('new conversation and source filter are icon buttons beside the search field', () => {
    const app = source();
    const css = styles();
    expect(app).toContain('className="session-searchbar"');
    expect(app).toContain('aria-label={t(\'chat.new\')}');
    expect(app).toContain('aria-pressed={props.hideCronSessions}');
    expect(app).toContain('setHideCronSessions={(value: boolean) => setHideCronSessions(value)}');
    expect(app).toContain('SlidersHorizontal');
    expect(app).toContain("session.source === 'turtle-soup' ? <Turtle /> : session.source === 'turtle-bench' ? <Turtle />");
    expect(css).toContain('.session-searchbar{display:grid;grid-template-columns:44px minmax(0,1fr) 44px');
    expect(css).toContain('.filter{height:44px');
    expect(css).toContain('.session-filter-btn{width:44px;height:44px');
  });

  test('session source filter copy names cron, CLI, alp-worker, and turtle conversations', () => {
    const i18n = translations();
    expect(i18n).toContain("en: 'Hide cron, CLI, alp-worker, turtle-soup, and turtle-bench conversations'");
    expect(i18n).toContain("'zh-CN': '隐藏定时任务、CLI、alp-worker、turtle-soup 和 turtle-bench 对话'");
    expect(i18n).toContain("en: 'Show cron, CLI, alp-worker, turtle-soup, and turtle-bench conversations'");
    expect(i18n).toContain("'zh-CN': '显示定时任务、CLI、alp-worker、turtle-soup 和 turtle-bench 对话'");
  });

  test('composer model comes from selected session details, not a global Hermes fallback', () => {
    const app = source();
    expect(app).toContain('activeSessionDetail');
    expect(app).toContain('loadSessionDetail(activeSessionId)');
    expect(app).toContain('const sessionModel = sessionModelOverride?.model || realModelOrEmpty(active?.model) || realModelOrEmpty(props.activeSessionDetail?.model) || realModelOrEmpty(props.model) || props.models[0]?.id ||');
    expect(app).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
    expect(app).toContain('const exactCurrentOption = currentModel ? findModelOption(props.models, currentModel, sessionProvider) : undefined;');
    expect(app).toContain('const currentOption = currentModel && !exactCurrentOption ? currentModelDisplayOption(currentModel, props.models, sessionProvider) : undefined;');
  });

  test('session rows keep missing titles as a dash instead of promoting preview text', () => {
    const app = source();
    expect(app).toContain('sessionDisplayTitle(session)');
  });

  test('active session sidebar preview is only changed by frontend during live streaming', () => {
    const app = source();
    expect(app).toContain("import { latestSessionPreviewFromMessages, sessionPreviewForDisplay } from './sessionPreview';");
    expect(app).toContain('if (streamingSessionId !== activeSessionId) return;');
    expect(app).toContain('const activePreview = latestSessionPreviewFromMessages(messages);');
    expect(app).toContain('setSessions((old) => old.map((session) => session.id === activeSessionId && session.preview !== activePreview ? { ...session, preview: activePreview } : session));');
    expect(app).toContain('setActiveSessionDetail((old) => old?.id === activeSessionId && old.preview !== activePreview ? { ...old, preview: activePreview } : old);');
    expect(app).toContain('setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, content: text, pending: true, timestamp: Date.now() / 1000 } : m));');
  });

  test('session row filters marker previews before rendering them', () => {
    const app = source();
    expect(app).toContain('sessionPreviewForDisplay(session.preview)');
  });

  test('session metadata refreshes preserve a known provider when a partial API row omits it', () => {
    const app = source();
    expect(app).toContain("if (!String(next.provider || '').trim() && String(current.provider || '').trim()) merged.provider = current.provider;");
  });

  test('chat provider identity uses only session metadata unless the session is a draft', () => {
    const app = source();
    expect(app).toContain("const messageProvider = latestMessageProviderForModel(props.messages, sessionModel);");
    expect(app).toContain("const apiSessionProvider = String(active?.provider || props.activeSessionDetail?.provider || messageProvider).trim();");
    expect(app).toContain("const sessionProvider = String(sessionModelOverride?.provider ?? (props.activeSessionId === DRAFT_SESSION_ID ? props.selectedModelProvider : apiSessionProvider)).trim();");
  });

  test('session list refreshes previews while the chat sidebar remains open', () => {
    const app = source();
    expect(app).toContain("const SESSION_LIST_REFRESH_INTERVAL_MS = 3000;");
    expect(app).toContain("if (mode !== 'chat') return;");
    expect(app).toContain('const timer = window.setInterval(() => {');
    expect(app).toContain('void loadSessions(filter);');
    expect(app).toContain('return () => window.clearInterval(timer);');
  });

  test('session polling keeps an in-flight assistant preview over a stale API row', () => {
    const app = source();
    expect(app).toContain("const livePreview = streamingSessionIdRef.current === session.id ? latestSessionPreviewFromMessages(messagesRef.current) : '';" );
    expect(app).toContain("const sessionForList = livePreview ? { ...session, preview: livePreview } : session;" );
    expect(app).toContain('sessionWithPreservedMessageCount(sessionForList, old.find((existing) => existing.id === session.id))');
  });

  test('session polling skips a tick while any list request is still running', () => {
    const app = source();
    expect(app).toContain('const sessionListRequestCountRef = useRef(0);');
    expect(app).toContain('sessionListRequestCountRef.current += 1;');
    expect(app).toContain('sessionListRequestCountRef.current = Math.max(0, sessionListRequestCountRef.current - 1);');
    expect(app).toContain('if (sessionListRequestCountRef.current > 0) return;');
  });

  test('opened session header delays stitched totals until the minimap response arrives', () => {
    const app = source();
    expect(app).toContain('const updateSessionMessageCount = useCallback((sessionId: string, total: unknown) => {');
    expect(app).toContain('sessionWithPreservedMessageCount(detail, old)');
    expect(app).toContain('sessionWithPreservedMessageCount(sessionForList, old.find((existing) => existing.id === session.id))');
    expect(app).toContain('updateSessionMessageCount(sessionId, page.total);');
    expect(app).toContain('updateSessionMessageCount(sessionId, body.total);');
    expect(app).toContain('if (Number.isFinite(total) && total >= 0) setHistoryTotal(Math.trunc(total));');
    expect(app).toContain("className={`chat-total-count${props.historyTotal === null ? ' loading' : ''}`}");
  });

  test('opened session header shows start and latest message times on the right', () => {
    const app = source();
    const css = styles();
    expect(app).toContain("import { sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';");
    expect(app).toContain("import { formatChatMessageTime } from './sessionTime';");
    expect(app).toContain('const headerTimes = sessionHeaderTimes(active, props.messages);');
    expect(app).toContain('className="session-header-times"');
    expect(css).toContain('.session-header-times{');
  });
});
