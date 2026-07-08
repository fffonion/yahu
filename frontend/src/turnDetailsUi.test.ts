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
    expect(source).toContain('aria-label={detailSummary}');
    expect(source).toContain('className="turn-detail-copy"');
    expect(source).toContain("const detailSummary = tf('chat.detailEntries', detailCount);");
    expect(source).not.toContain('Tools & thinking');
    expect(source).not.toContain('Turn tools and thinking');
  });

  test('outer turn detail summary is a single long bar with only a right chevron glyph', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("<summary className=\"turn-detail-summary\"><span className=\"turn-detail-copy\">{detailSummary}</span><ChevronRight className=\"tool-chevron turn-detail-arrow\" aria-hidden=\"true\" /></summary>");
    expect(source).not.toContain('turn-detail-toggle');
    expect(source).not.toContain('turn-detail-toggle-label');
    expect(source).not.toContain("t('chat.expandDetails')");
    expect(source).not.toContain("t('chat.collapseDetails')");
    expect(styles).toContain('.turn-detail-summary{display:grid;grid-template-columns:minmax(0,1fr) auto;');
    expect(styles).toContain('.turn-detail-summary::-webkit-details-marker{display:none}');
    expect(styles).toContain('.turn-detail-arrow{justify-self:end}');
    expect(styles).toContain('.tool-chevron{width:15px;height:15px;color:var(--muted);transition:transform .15s ease}');
    expect(styles).toContain('.turn-detail-group[open] .turn-detail-arrow{transform:rotate(90deg)}');
    expect(source).toContain("aria-label={detailSummary} onToggle={(event) => { const nextOpen = event.currentTarget.open; setOpen(nextOpen); if (nextOpen) loadDetails(); }}");
    expect(source).not.toContain("open={open} onToggle=");
    expect(source).not.toContain("defaultOpen=");
    expect(source).toContain("const detailAnchorId = String(item.messages[0]?.id || item.id);");
    expect(source).toContain('data-message-id={!open ? detailAnchorId : undefined}');
    expect(source).toContain('suppressMessageAnchor={!open}');
    expect(source).toContain('data-message-id={!suppressMessageAnchor ? message.id || undefined : undefined}');
  });

  test('normal and compact chat paths both render the same turn detail group component', () => {
    const source = app();
    const compactIndex = source.indexOf('if (props.desktopCompactMessages)');
    const compactGroupIndex = source.indexOf('<DesktopTurnBlock block={block}', compactIndex);
    const normalMapIndex = source.indexOf('return turnDetailItems.map((item) => {', compactIndex);
    const normalGroupIndex = source.indexOf('<TurnDetailGroup item={item}', normalMapIndex);
    expect(compactIndex).toBeGreaterThan(-1);
    expect(compactGroupIndex).toBeGreaterThan(compactIndex);
    expect(normalMapIndex).toBeGreaterThan(compactGroupIndex);
    expect(normalGroupIndex).toBeGreaterThan(normalMapIndex);
  });

  test('outer turn detail group has compact collapsed styling separate from inner tool and reasoning folds', () => {
    const styles = css();
    expect(styles).toContain('.turn-detail-group');
    expect(styles).toContain('.turn-detail-summary');
    expect(styles).toContain('.turn-detail-group{display:grid;width:100%;max-width:920px;align-self:flex-start;');
    expect(styles).toContain('.turn-detail-group:not([open]) .turn-detail-body{display:none}');
    expect(styles).toContain('.turn-detail-body .tool-summary');
    expect(styles).toContain('.turn-detail-body .msg-reasoning');
    expect(styles).toContain('.desktop-compact-chat .turn-detail-body,.mobile-compact-chat .turn-detail-body{padding:10px 12px 12px}');
  });

  test('compact live streaming detail columns match completed turn details on desktop and mobile', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("return <main className={`main-panel chat-main-panel ${props.desktopCompactMessages ? 'desktop-compact-chat' : ''}${isMobile ? ' mobile-compact-chat' : ''}`}>"
    );
    expect(styles).toContain('.desktop-compact-chat .tool-detail,.mobile-compact-chat .tool-detail{border-top:0;background:transparent;padding:4px 0 0 38px;max-height:none}');
    expect(styles).toContain('.desktop-compact-chat .msg-reasoning pre,.mobile-compact-chat .msg-reasoning pre{padding-left:21px}');
    expect(styles).not.toContain('.desktop-compact-chat .turn-detail-body .tool-detail{padding:4px 0 0 38px}');
    expect(styles).not.toContain('.mobile-compact-chat .turn-detail-body .tool-detail{padding:4px 0 0 38px}');
    expect(styles).not.toContain('.desktop-compact-chat .turn-detail-body .msg-reasoning pre{padding-left:21px}');
    expect(styles).not.toContain('.mobile-compact-chat .turn-detail-body .msg-reasoning pre{padding-left:21px}');
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

  test('special context block has collapsed details styling and i18n copy', () => {
    const source = app();
    expect(source).toContain('import { buildDesktopTurnBlocks, buildTurnDetailItems, type SpecialContextGroupItem, type TurnDetailBlock, type TurnDetailGroupItem, type TurnDetailMetadata }');
    expect(source).toContain('function SpecialContextGroup');
    expect(source).toContain('className="special-context-block"');
    expect(source).toContain('className="special-context-summary"');
    expect(source).toContain('className="special-context-copy"');
    expect(source).toContain('className="special-context-body"');
    expect(source).toContain("className=\"tool-chevron special-context-arrow\"");
    expect(source).toContain("const title = t('chat.specialContext');");
    expect(source).toContain('<SpecialContextGroup item={item} />');
  });

  test('streaming path still appends full detail messages instead of skeleton-only rows', () => {
    const source = app();
    expect(source).toContain("if (createdSession) setMessages(() => [userMsg, assistantMsg]);");
    expect(source).toContain("else setMessages((old) => [...old, userMsg, assistantMsg].slice(-MESSAGE_WINDOW));");
    expect(source).toContain('fetch(`/chat/stream/${encodeURIComponent(sessionId)}`');
    expect(source).not.toContain('/chat/stream/${encodeURIComponent(sessionId)}?view=skeleton');
  });
});
