import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const card = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
const transcript = () => {
  const path = new URL('./ChatTranscript.tsx', import.meta.url);
  return existsSync(path) ? readFileSync(path, 'utf8') : '';
};

describe('subagent progress UI', () => {
  test('uses one live websocket and cancellable historical snapshots for the visible time window', () => {
    expect(app()).toContain("import { SubagentProgressCard } from './SubagentProgressCard';");
    expect(app()).toContain('<SubagentProgressCard sessionId={props.activeSessionId} beforeTime={subagentBeforeTime} showReasoning={props.showReasoning} showToolCalls={props.showToolCalls} compact={props.desktopCompactMessages} />');
    expect(card()).toContain('new WebSocket(subagentWebSocketUrl(window.location, sessionId))');
    expect(card()).toContain('fetch(subagentSnapshotUrl(sessionId, beforeTime), { signal: controller.signal })');
    expect(card()).toContain('return () => { requestGuard.stop(); controller.abort(); };');
    expect(card()).toContain('if (!requestGuard.isActive(controller.signal)) return;');
    expect(card()).toContain("type: 'subagents.snapshot',");
    const cardSource = card();
    const historicalRefresh = cardSource.slice(
      cardSource.indexOf("if (typeof beforeTime === 'number')"),
      cardSource.indexOf('let socket: WebSocket | null = null'),
    );
    expect(historicalRefresh).not.toContain('subagents: [], error: undefined');
    expect(historicalRefresh).toContain('setProjectionPending(true);');
    expect(cardSource).not.toContain('current ? { ...current, subagents: []');
    expect(cardSource).toContain('current ? { ...current, error: undefined }');
    expect(cardSource).toContain('current ? { ...current, error: String(error) }');
    expect(cardSource).toContain('snapshot.error || projectionPending');
    expect(cardSource).toContain("projectionPending ? t('subagents.refreshing')");
    expect(card()).toContain('}, [sessionId]);');
    expect(app()).toContain('subagentBeforeTimeForVisibleRange(props.chatScrollRef.current, props.messages, props.hasNewer)');
    expect(app()).toContain('subagentPrecedingFallbackIds(rows.map((row) => {');
    expect(app()).toContain('scheduleSubagentWindowUpdate();');
    expect(app()).toContain('new ResizeObserver(scheduleSubagentWindowUpdate)');
    expect(app()).toContain('observer.observe(scroller);');
    expect(app()).toContain('}, 150);');
    expect(card()).toContain('normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId)');
    expect(card()).toContain("node.ancestryOmitted && <span className=\"subagent-progress-omitted-ancestry\" title={t('subagents.parentOmitted')}");
  });

  test('keeps todos and shared conversation detail while removing counters, model, and recent activity chrome', () => {
    const source = card();
    expect(source).toContain('className="subagent-progress-tree"');
    expect(source).toContain('subagent-progress-todos${className');
    expect(source).toContain('className="subagent-progress-messages"');
    expect(source).toContain('formatSubagentFinalMessages(messages)');
    expect(source).toContain('structuredContent: parseSubagentFinalStructuredContent(node.summary)');
    expect(source).not.toContain('prettyFormatSubagentFinalMessage');
    expect(source).toContain('<SubagentProgressNode key={child.sessionId}');
    expect(source).not.toContain('className="subagent-progress-stats"');
    expect(source).not.toContain('className="subagent-progress-activity"');
    expect(source).not.toContain('assistantName={node.model');
  });

  test('lazy-loads full conversation detail while keeping only one child expanded across snapshot replacements', () => {
    const source = card();
    expect(source).toContain("const [openNodeIds, setOpenNodeIds] = useState<Set<string>>(() => new Set());");
    expect(source).toContain('openNodeIds={openNodeIds}');
    expect(source).toContain('const open = openNodeIds.has(node.sessionId);');
    expect(source).toContain('onOpenChange={setNodeOpen}');
    expect(source).toContain('setOpenNodeIds((current) => {');
    expect(source).toContain('if (open) return new Set([nodeSessionId]);');
    expect(source).not.toContain('if (open) next.add(nodeSessionId);');
    expect(source).toContain('setOpenNodeIds(new Set());');
    expect(source).toContain('const [detailCache, setDetailCache]');
    expect(source).toContain('detailCache={detailCache}');
    expect(source).toContain('onMessagesLoaded={cacheNodeMessages}');
    expect(source).toContain('const cachedDetail = detailCache[node.sessionId];');
    expect(source).toContain('onMessagesLoaded(node.sessionId, node.messageCount, items);');
    expect(source).toContain("event.preventDefault(); const nextOpen = !open; onOpenChange(node.sessionId, nextOpen);");
    expect(source).not.toContain('<details open={open} onToggle=');
    expect(source).toContain('subagentMessagesUrl(node.sessionId)');
    expect(source).toContain('normalizeSubagentMessages(await response.json())');
    expect(source).toContain('className="subagent-progress-messages"');
    expect(source).toContain('<ChatTranscript');
    expect(source).not.toContain('const [open, setOpen] = useState(false);');
    expect(source).not.toContain("useState(node.status === 'running')");
  });

  test('compresses completed agents to one check-and-description line while keeping status and time inside the expanded detail', () => {
    const source = card();
    const styles = css();
    expect(source).toContain("const completed = node.status === 'completed';");
    expect(source).toContain("className={completed ? 'completed' : undefined}");
    expect(source).toContain("{!completed && <small>{statusLabel(node.status)} · {elapsed}{node.currentTool ? ` · ${node.currentTool}` : ''}</small>}");
    expect(source).toContain('{completed && <p className="subagent-progress-detail-meta">{statusLabel(node.status)} · {elapsed}</p>}');
    expect(source).toContain("${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}");
    expect(source).toContain("<strong>{completed ? node.task : t('subagents.title')}</strong>");
    expect(source).toContain("aria-label={`${completed ? node.task : t('subagents.title')}: ${statusLabel(node.status)}`}");
    expect(styles).toContain('.subagent-progress-card.collapsed.completed-preview .subagent-progress-heading strong{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px;font-weight:400}');
    expect(styles).toContain('.subagent-progress-node>details>summary.completed{grid-template-columns:auto minmax(0,1fr);min-height:36px;');
    expect(styles).toContain('.subagent-progress-card.collapsed.completed-preview .subagent-progress-panel-toggle{grid-template-columns:auto minmax(0,1fr);min-height:40px;');
    expect(styles).toContain('.subagent-progress-node>details[open]>summary.completed .subagent-progress-goal strong{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}');
  });

  test('reuses the exact main-chat transcript renderer without a subagent-only message renderer', () => {
    expect(app()).toContain("import { ChatTranscript");
    expect(app()).toContain('<ChatTranscript');
    expect(card()).toContain("import { ChatTranscript");
    expect(card()).toContain('showReasoning={showReasoning}');
    expect(card()).toContain('showToolCalls={showToolCalls}');
    expect(app()).toContain('showReasoning={props.showReasoning} showToolCalls={props.showToolCalls}');
    expect(card()).not.toContain('function SubagentConversationMessage');
    expect(card()).not.toContain('subagent-progress-tool-calls');
    expect(transcript()).toContain('function ToolMessageView');
    expect(transcript()).toContain('buildTurnDetailItems');
    expect(transcript()).toContain('buildDesktopTurnBlocks');
    expect(app()).not.toContain('function ToolMessageView');
    expect(app()).not.toContain('function MessageView');
    expect(css()).not.toContain('.subagent-progress-message{');
    expect(css()).not.toContain('.subagent-progress-tool-calls{');
  });

  test('shows a persisted goal separately while keeping the running subagent card visible', () => {
    const source = card();
    const styles = css();
    expect(source).toContain('const [goalExpanded, setGoalExpanded] = useState(false);');
    expect(source).toContain('setGoalExpanded(false);');
    expect(source).toContain('className="subagent-progress-stack"');
    expect(source).toContain('className="subagent-goal-panel" open={goalExpanded}');
    expect(source).toContain('<span className="subagent-status-icon subagent-goal-icon"><Target aria-hidden="true" /></span>');
    expect(source).not.toContain("<strong>{t('subagents.goal')}</strong>");
    expect(source).toContain('const goal = snapshot.goal;');
    expect(source).not.toContain('className="subagent-goal-text">{goal.text}</div>');
    expect(source).toContain('<GoalMilestones goal={goal} />');
    expect(source).toContain("const milestones = [...goal.milestones].sort((left, right) => (right.timestamp || 0) - (left.timestamp || 0) || right.turn - left.turn);");
    expect(styles).toContain('.subagent-goal-milestones li p{margin:0;color:color-mix(in srgb,var(--text) 92%,var(--subagent-goal-accent));');
    expect(styles).toContain('.subagent-goal-body{max-height:min(62vh,720px);');
    expect(styles).not.toContain('.subagent-goal-reason');
    expect(source).toContain('<SubagentTodoList todos={goal.todos} className="subagent-goal-todos" />');
    expect(source).toContain('<SubagentTodoList todos={node.todos} />');
    expect(source).toContain("function SubagentTodoList({ todos, className = '' }");
    expect(source).not.toContain('shouldShowSubagentPanel');
    expect(source).toContain('{(snapshot.subagents.length > 0 || snapshot.error || projectionPending) && <section className={`subagent-progress-card');
    expect(styles).toContain('.subagent-goal-icon{color:var(--subagent-goal-accent);background:color-mix(in srgb,var(--subagent-goal-accent) 12%,transparent)}');
    expect(source).toContain('const goalMetadata = goal ? [');
    expect(source).toContain("tf('goals.turnProgress', goal.turnsUsed, goal.maxTurns)");
    expect(source).toContain('className="subagent-goal-meta">{goalMetadata}</small>');
    expect(source).toContain('</div>\n      <footer className="subagent-goal-footer">{goalMetadata}</footer>');
    expect(styles).toContain('.subagent-goal-copy{min-width:0;display:grid;gap:3px}');
    expect(styles).toContain('.subagent-goal-meta{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;');
    expect(styles).toContain('.subagent-goal-panel[open] .subagent-goal-preview{white-space:normal;overflow:visible;text-overflow:clip;font-size:13px}');
    expect(styles).toContain('.subagent-goal-body{max-height:min(62vh,720px);overflow-y:auto;overscroll-behavior:contain;padding:10px 12px;border-top:1px solid color-mix(in srgb,var(--border) 82%,transparent);font-size:14px;');
    expect(styles).toContain('.subagent-goal-todos li{font-size:12px}');
    expect(styles).toContain('.subagent-goal-milestones>header{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:8px;color:color-mix(in srgb,var(--text) 88%,var(--subagent-goal-accent));font-size:13px;');
    expect(styles).toContain('.subagent-goal-milestone-meta{display:flex;align-items:center;justify-content:space-between;gap:8px;color:color-mix(in srgb,var(--subagent-goal-accent) 82%,var(--text));font-size:11px;');
    expect(styles).toContain('.subagent-goal-milestone-meta time{font:500 11px/1.2 var(--mono)}');
    expect(styles).toContain('.subagent-goal-milestones li p{margin:0;color:color-mix(in srgb,var(--text) 92%,var(--subagent-goal-accent));font-size:13px;');
    expect(styles).toContain('.subagent-goal-footer{margin:0;padding:9px 12px;border-top:1px solid color-mix(in srgb,var(--subagent-goal-accent) 28%,var(--border));background:color-mix(in srgb,var(--subagent-goal-accent) 5%,transparent);color:color-mix(in srgb,var(--subagent-goal-accent) 78%,var(--text));font-size:11px;');
    expect(styles).toContain('.subagent-goal-panel[open] .subagent-goal-meta{display:none}');
    expect(i18n()).toContain("'goals.active': { en: 'Active', 'zh-CN': '进行中'");
    expect(i18n()).toContain("'goals.turnProgress': { en: '{0}/{1} turns', 'zh-CN': '{0}/{1} 轮'");
    expect(source).toContain("if (!snapshot || snapshot.sessionId !== sessionId || (!snapshot.goal && !snapshot.subagents.length && !snapshot.error && !projectionPending)) return null;");
    expect(source).toContain('socket.onmessage = (event) => {\n        if (stopped) return;');
    expect(source).toContain("aria-label={`${completed ? node.task : t('subagents.title')}: ${statusLabel(node.status)}`}");
    expect(source.indexOf('className="subagent-goal-panel"')).toBeLessThan(source.indexOf('className={`subagent-progress-card'));
    expect(source).toContain('<span className="subagent-progress-heading"><strong>{t(\'subagents.title\')}</strong>');
    expect(styles).toContain('.subagent-progress-stack{width:100%;max-width:none;min-height:0;max-height:90%;display:flex;flex-direction:column;align-items:stretch;gap:6px;pointer-events:none;');
    expect(styles).toContain('.subagent-goal-panel{--subagent-goal-accent:var(--accent-2);width:100%;flex:0 0 auto;pointer-events:auto;');
    expect(styles).toContain('color:var(--subagent-goal-accent)');
    expect(styles).toContain('.subagent-goal-panel[open] .subagent-goal-chevron{transform:rotate(90deg)}');
    expect(styles).toContain('.subagent-goal-body{');
    expect(styles).toContain('.subagent-goal-subgoals{margin:10px 0 0;padding:10px 0 0 20px;border-top:1px solid');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:44px}');
  });

  test('uses a responsive card that remains readable in compact desktop and mobile chat', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-card{');
    expect(styles).toContain('.subagent-progress-node>details>summary{');
    expect(styles).toContain('.subagent-progress-node>details[open]>summary .subagent-progress-goal strong{white-space:normal;overflow:visible;text-overflow:clip}');
    expect(styles).toContain('@media(max-width:760px){.subagent-progress-card');
    expect(styles).toContain('.desktop-compact-chat .subagent-progress-card');
  });

  test('fills the chat panel edge to edge with rounded outer card corners on desktop and mobile', () => {
    const styles = css();
    const cardRule = styles.match(/\.subagent-progress-card\{([^}]*)\}/)?.[1] || '';
    const overlayRule = styles.match(/\.subagent-progress-overlay\{([^}]*)\}/)?.[1] || '';
    expect(cardRule).toContain('width:100%');
    expect(cardRule).toContain('max-width:none');
    expect(cardRule).toContain('border-radius:14px');
    expect(overlayRule).toContain('padding:0');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:44px}.subagent-progress-overlay{padding:0}');
    expect(styles).not.toContain('.subagent-progress-card.expanded{height:90%;max-height:90%;border-radius:');
    expect(styles).not.toContain('.subagent-progress-card.collapsed{border-radius:');
  });

  test('keeps shared transcript reasoning folds out of subagent node chrome', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-node>details{');
    expect(styles).toContain('.subagent-progress-node>details>summary{');
    expect(styles).not.toContain('.subagent-progress-node details{');
    expect(styles).not.toContain('.subagent-progress-node summary{');
    expect(styles).toContain('.msg-reasoning>summary{cursor:pointer;list-style:none;display:flex;align-items:center;gap:6px}');
  });

  test('keeps the subagent conversation detail frame square like the shared chat detail frame', () => {
    const styles = css();
    const subagentDetailRule = styles.match(/\.subagent-progress-node>details\{([^}]*)\}/)?.[1] || '';
    const sharedDetailRule = styles.match(/\.turn-detail-group\{([^}]*)\}/)?.[1] || '';
    expect(sharedDetailRule).toContain('border-radius:0');
    expect(subagentDetailRule).toContain('border-radius:0');
    expect(subagentDetailRule).not.toContain('border-radius:12px');
  });

  test('keeps a newly opened streaming detail at its latest content through the outer tree container', () => {
    const source = card();
    const styles = css();
    expect(source).toContain('const detailTreeRef = useRef<HTMLDivElement>(null);');
    expect(source).toContain('const followLatestDetailRef = useRef(true);');
    expect(source).toContain('tree.scrollTop = tree.scrollHeight;');
    expect(source).toContain('followLatestDetailRef.current = isSubagentDetailNearBottom(event.currentTarget);');
    expect(source).toContain('onDetailOpen={startFollowingLatestDetail}');
    expect(source).toContain('onDetailContentChange={followLatestDetail}');
    expect(source).toContain('if (open && detailMessages.length > 0) onDetailContentChange();');
    expect(source).not.toContain('const detailRef = useRef<HTMLDivElement>(null);');
    expect(source).not.toContain('className="subagent-progress-detail" ref={detailRef}');
    expect(styles).toContain('.subagent-progress-panel-body .subagent-progress-tree{min-height:0;flex:1 1 auto;overflow-y:auto;overscroll-behavior:contain;touch-action:pan-y;-webkit-overflow-scrolling:touch;');
    expect(styles).not.toContain('.subagent-progress-detail{min-height:0;overflow-y:auto');
    expect(styles).not.toContain('.subagent-progress-node>details[open]{display:grid;grid-template-rows:auto minmax(0,1fr);max-height:');
  });

  test('floats above chat history and grows naturally up to ninety percent height', () => {
    const appSource = app();
    const cardSource = card();
    const styles = css();
    const overlayIndex = appSource.indexOf('className="subagent-progress-overlay"');
    const chatScrollIndex = appSource.indexOf('className="chat-scroll"');
    expect(overlayIndex).toBeGreaterThan(-1);
    expect(chatScrollIndex).toBeGreaterThan(overlayIndex);
    expect(cardSource).toContain('const [expanded, setExpanded] = useState(false);');
    expect(cardSource).toContain('const preview = previewSubagent(snapshot.subagents);');
    expect(cardSource).toContain('aria-expanded={expanded}');
    expect(cardSource).toContain("className={`subagent-progress-card ${expanded ? 'expanded' : 'collapsed'}${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}`}");
    expect(cardSource).toContain('{!expanded && preview && <SubagentProgressPreview');
    expect(cardSource).toContain('{expanded && <div className="subagent-progress-panel-body">');
    expect(styles).toContain('.subagent-progress-overlay{grid-row:2;grid-column:1;min-width:0;min-height:0;z-index:140;');
    expect(styles).toContain('.chat-main-panel .chat-scroll{grid-row:2;grid-column:1;');
    expect(styles).toContain('.chat-main-panel>.composer-wrap{grid-row:3;grid-column:1}');
    expect(styles).toContain('background:linear-gradient(145deg,color-mix(in srgb,var(--accent) 9%,var(--surface))');
    expect(styles).not.toContain('background:linear-gradient(145deg,color-mix(in srgb,var(--accent-soft) 46%,var(--surface))');
    expect(styles).toContain('.subagent-progress-card.expanded{max-height:90%;');
    expect(styles).not.toContain('.subagent-progress-card.expanded{height:90%');
    expect(styles).not.toContain('.subagent-progress-card.expanded{height:90%;max-height:90%');
    expect(styles).toContain('.subagent-progress-panel-body{min-height:0;');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:44px}.subagent-progress-overlay{padding:0}');
    expect(styles).not.toContain('@media(max-width:760px){.subagent-goal-summary{min-height:44px}.subagent-progress-overlay{padding:0}.subagent-progress-card.expanded{height:90%');
  });
});
