import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('turn detail fold UI', () => {
  test('chat history renders final-turn intermediate tool and thinking rows inside a second-level collapsed details group', () => {
    const source = app();
    expect(source).toContain("buildTurnDetailItems");
    expect(source).toContain("<TurnDetailGroup");
    expect(source).toContain('className="turn-detail-group"');
    expect(source).toContain('className="turn-detail-summary"');
    expect(source).toContain('aria-label={t(\'chat.details\')}');
    expect(source).toContain('<span className="turn-detail-title">{t(\'chat.details\')}</span>');
    expect(source).not.toContain('Tools & thinking');
    expect(source).not.toContain('Turn tools and thinking');
  });

  test('outer turn detail summary exposes a visible translated expand control', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="turn-detail-toggle"');
    expect(source).toContain("<span className=\"turn-detail-toggle-label\">{open ? t('chat.collapseDetails') : t('chat.expandDetails')}</span>");
    expect(source).not.toContain('<span className="turn-detail-toggle-label">Expand</span>');
    expect(styles).toContain('.turn-detail-toggle{display:inline-grid;grid-template-columns:14px auto;');
    expect(source).toContain("open={open} onToggle={(event) => { const nextOpen = event.currentTarget.open; setOpen(nextOpen); if (nextOpen) loadDetails(); }}");
    expect(source).toContain("const detailAnchorId = String(item.messages[0]?.id || item.id);");
    expect(source).toContain('data-message-id={!open ? detailAnchorId : undefined}');
    expect(source).toContain('suppressMessageAnchor={!open}');
    expect(source).toContain('data-message-id={!suppressMessageAnchor ? message.id || undefined : undefined}');
  });

  test('outer turn detail group has compact collapsed styling separate from inner tool and reasoning folds', () => {
    const styles = css();
    expect(styles).toContain('.turn-detail-group');
    expect(styles).toContain('.turn-detail-summary');
    expect(styles).toContain('.turn-detail-group:not([open]) .turn-detail-body{display:none}');
    expect(styles).toContain('.turn-detail-body .tool-summary');
    expect(styles).toContain('.turn-detail-body .msg-reasoning');
    expect(styles).toContain('.desktop-compact-chat .turn-detail-body{padding:10px 12px 12px}');
  });

  test('history pages request skeleton rows and detail groups lazy-load their messages on expand', () => {
    const source = app();
    expect(source).toContain("const params = new URLSearchParams({ limit: String(MESSAGE_PAGE), view: 'skeleton' });");
    expect(source).toContain("const params = new URLSearchParams({ limit: String(MESSAGE_PAGE * 2), view: 'skeleton' });");
    expect(source).toContain("detailParams.set('view', 'details');");
    expect(source).toContain("fetch(`/chat/messages/${encodeURIComponent(sessionId)}?${detailParams}`)");
    expect(source).toContain("const detailMessages = useMemo(() => loadedMessages.length ? visibleChatMessages<ChatMessage>(loadedMessages, showReasoning, true) : item.messages");
    expect(source).toContain("loading ? t('status.loading')");
  });

  test('streaming path still appends full detail messages instead of skeleton-only rows', () => {
    const source = app();
    expect(source).toContain("if (createdSession) setMessages(() => [userMsg, assistantMsg]);");
    expect(source).toContain("else setMessages((old) => [...old, userMsg, assistantMsg].slice(-MESSAGE_WINDOW));");
    expect(source).toContain('fetch(`/chat/stream/${encodeURIComponent(sessionId)}`');
    expect(source).not.toContain('/chat/stream/${encodeURIComponent(sessionId)}?view=skeleton');
  });
});
