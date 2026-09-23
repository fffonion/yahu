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
    expect(app()).toContain("import { SubagentProgressStack } from './SubagentProgressCard';");
    expect(app()).toContain('<SubagentProgressStack sessionId={props.activeSessionId} beforeTime={subagentBeforeTime}');
    expect(card()).toContain('new WebSocket(subagentWebSocketUrl(window.location, sessionId))');
    expect(card()).toContain('fetch(subagentSnapshotUrl(sessionId, beforeTime), { signal: controller.signal })');
    expect(card()).toContain('return () => { requestGuard.stop(); controller.abort(); };');
    expect(card()).toContain('if (!requestGuard.isActive(controller.signal)) return;');
    expect(card()).toContain('const placeholder: SubagentProgressSnapshot = {');
    const cardSource = card();
    const historicalRefresh = cardSource.slice(
      cardSource.indexOf("if (typeof beforeTime === 'number')"),
      cardSource.indexOf('let socket: WebSocket | null = null'),
    );

    expect(historicalRefresh).toContain('setProjectionPending(true);');

    expect(cardSource).toContain('const currentForSession = current?.sessionId === sessionId ? current : cached;');
    expect(cardSource).toContain('currentForSession ? { ...currentForSession, error: String(error) }');

    expect(cardSource).toContain('visibleSnapshot.subagents.length > 0 || visibleSnapshot.error');
    expect(cardSource).toContain("projectionPending || total === 0 ? t('subagents.refreshing')");
    expect(cardSource).toContain('!selectedNode && !projectionPending && total > 0 ?');
    expect(card()).toContain('}, [sessionId]);');
    expect(app()).toContain('subagentBeforeTimeForVisibleRange(props.chatScrollRef.current, props.messages, props.hasNewer)');
    expect(app()).toContain('subagentPrecedingFallbackIds(rows.map((row) => {');
    expect(app()).toContain('scheduleSubagentWindowUpdate();');
    expect(app()).toContain('new ResizeObserver(scheduleSubagentWindowUpdate)');
    expect(app()).toContain('window.setTimeout(() => { updateActiveNavigatorIds(); updateSubagentWindow(); }, 360);');
    expect(app()).toContain('resizeObserver.observe(scroller);');
    expect(app()).toContain('}, 150);');
    expect(card()).toContain('normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId)');
    expect(card()).not.toContain('subagents.parentOmitted');
    expect(card()).not.toContain('subagent-progress-omitted-ancestry');
  });

  test('hands wheel scrolling from an expanded status detail back to the main chat at its edges', () => {
    const source = app();
    expect(source).toContain('onWheel={onStatusOverlayWheel}');
    expect(source).toContain("target.closest('.subagent-progress-tree')");
    expect(source).toContain('shouldForwardSubagentWheel(detailTree, event.deltaY)');
    expect(source).toContain('scroller.scrollTop = Math.min(');
    expect(source).toContain('scroller.scrollTop + event.deltaY');
    expect(source).toContain('event.preventDefault();');
  });

  test('clicking chat history collapses Goal and every subagent status sheet', () => {
    const appSource = app();
    const cardSource = card();
    expect(appSource).toContain('const [statusBarCollapseToken, setStatusBarCollapseToken] = useState(0);');
    expect(appSource).toContain('onClick={onChatAreaClick}');
    expect(appSource).toContain('setStatusBarCollapseToken((value) => value + 1);');
    expect(appSource).toContain('collapseToken={statusBarCollapseToken}');
    expect(cardSource).toContain('collapseToken?: number;');
    expect(cardSource).toContain('if (collapseToken === undefined) return;');
    expect(cardSource).toContain('setExpanded(false);');
    expect(cardSource).toContain('setGoalExpanded(false)');
    expect(cardSource).toContain('collapseToken={collapseToken}');
  });

  test('keeps cached status sheets visible while refreshing without replacing unchanged snapshots', () => {
    const source = card();
    expect(source).toContain("import { readCachedSubagentSnapshot, sameSubagentSnapshot, writeCachedSubagentSnapshot } from './subagentSnapshotCache';");
    expect(source).toContain('setSnapshot(useSessionCache ? readCachedSubagentSnapshot(sessionId) : null);');
    expect(source).toContain('const visibleSnapshot = snapshot?.sessionId === sessionId ? snapshot : cachedSnapshot;');
    expect(source).toContain('if (!cached) {');
    expect(source).toContain('writeCachedSubagentSnapshot(next);');
    expect(source).toContain('return currentForSession && sameSubagentSnapshot(currentForSession, next) ? currentForSession : next;');
    expect(source).not.toContain('setSnapshot(null);');
  });

  test('status sheets stack square-topped with a subtle bottom shadow', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-stack{width:100%;max-width:none;min-height:0;max-height:90%;display:flex;flex-direction:column;align-items:stretch;gap:0;pointer-events:none;box-sizing:border-box}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel,.subagent-progress-stack>.subagent-progress-card{border-radius:0 0 var(--radius-card) var(--radius-card)}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel{z-index:3}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-progress-card{z-index:1}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-progress-card:first-of-type{z-index:2}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-progress-card+.subagent-progress-card{margin-top:calc(-1 * var(--radius-card) - 2px);padding-top:calc(var(--radius-card) + 2px)}');
    expect(styles).toContain('box-shadow:0 5px 14px rgba(0,0,0,.30)');
    expect(styles).not.toContain('box-shadow:0 4px 10px rgba(0,0,0,.12)');
    expect(styles).not.toContain('box-shadow:0 3px 8px rgba(0,0,0,.10)');
    expect(styles).not.toContain('0 12px 34px rgba(0,0,0,.09)');
    expect(styles).not.toContain('0 8px 24px rgba(0,0,0,.07)');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel+.subagent-progress-card{margin-top:calc(-1 * var(--radius-card) - 2px);padding-top:calc(var(--radius-card) + 2px)}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel+.subagent-progress-card.expanded{padding-top:calc(var(--radius-card) + 15px)}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel+.subagent-progress-card.collapsed .subagent-progress-panel-toggle{min-height:56px}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel .subagent-goal-summary{min-height:56px}');
    expect(styles).not.toContain('overflow-clip-margin:18px');
  });

  test('keeps todos and shared conversation detail while showing model identity', () => {
    const source = card();
    expect(source).toContain('className="subagent-progress-tree"');
    expect(source).toContain('subagent-progress-todos${className');
    expect(source).toContain('className="subagent-progress-messages"');
    expect(source).toContain('formatSubagentFinalMessages(subagentDetailMessages(messages, node.context))');
    expect(source).toContain('structuredContent: parseSubagentFinalStructuredContent(node.summary)');

    expect(source).toContain('const displayTree = useMemo(() => latestSubagentRows(tree), [tree]);');
    expect(source).toContain('<SubagentProgressNode key={node.sessionId}');
    expect(source).not.toContain('subagent-progress-children');
    expect(css()).not.toContain('.subagent-progress-children');

    expect(source).toContain('className="subagent-progress-model"');
    expect(source).toContain('assistantName={node.model}');
  });

  test('shows completion age and duration before the model for completed subagents', () => {
    const source = card();
    expect(source).toContain('formatSubagentCompletionAge');
    expect(source).toContain('formatSubagentDurationCompact');
    expect(source).toContain('function completedSubagentSubtitle(node: SubagentProgress, nowSeconds: number)');
    expect(source).toContain("if (node.status !== 'completed') return '';");
    expect(source).toContain('completedSubagentSubtitle(node, nowSeconds)');
    expect(i18n()).toContain("'subagents.completedAgo': { en: '{0} ago', 'zh-CN': '{0}前', 'zh-TW': '{0}前', ja: '{0}前' }");
  });

  test('lazy-loads full conversation detail while keeping only one child expanded across snapshot replacements', () => {
    const source = card();
    expect(source).toContain("const [openNodeIds, setOpenNodeIds] = useState<Set<string>>(() => new Set());");
    expect(source).toContain('openNodeIds={openNodeIds}');
    expect(source).toContain('const open = openNodeIds.has(node.sessionId);');
    expect(source).toContain('onOpenChange={setNodeOpen}');
    expect(source).toContain('setOpenNodeIds((current) => {');
    expect(source).toContain('if (open) return new Set([nodeSessionId]);');

    expect(source).toContain('setOpenNodeIds(new Set());');
    expect(source).toContain('const [detailCache, setDetailCache]');
    expect(source).toContain('detailCache={detailCache}');
    expect(source).toContain('onMessagesLoaded={cacheNodeMessages}');
    expect(source).toContain('const cachedDetail = detailCache[node.sessionId];');
    expect(source).toContain('onMessagesLoaded(node.sessionId, node.messageCount, items);');
    expect(source).toContain("event.preventDefault(); const nextOpen = !open; onOpenChange(node.sessionId, nextOpen);");

    expect(source).toContain('subagentMessagesUrl(node.sessionId)');
    expect(source).toContain('normalizeSubagentMessages(await response.json())');
    expect(source).toContain('className="subagent-progress-messages"');
    expect(source).toContain('<ChatTranscript');

  });

  test('compresses completed agents to one check-and-description line while keeping status and time inside the expanded detail', () => {
    const source = card();
    const styles = css();
    expect(source).toContain("const completed = node.status === 'completed';");
    expect(source).toContain("className={completed ? 'completed' : undefined}");
    expect(source).toContain('className="subagent-progress-model"');
    expect(source).toContain('{completed && <p className="subagent-progress-detail-meta">{statusLabel(node.status)} · {elapsed}</p>}');
    expect(source).toContain("${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}");
    expect(source).toContain("<strong>{completed ? node.task : t('subagents.title')}</strong>");
    expect(source).toContain("aria-label={`${completed ? node.task : t('subagents.title')}: ${statusLabel(node.status)}`}");
    expect(styles).toContain('.subagent-progress-card.collapsed.completed-preview .subagent-progress-heading strong{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px;font-weight:400}');
    expect(styles).toContain('.subagent-progress-node>details>summary.completed{grid-template-columns:auto minmax(0,1fr);min-height:36px;');
    expect(styles).toContain('.subagent-progress-card.collapsed.completed-preview .subagent-progress-panel-toggle{grid-template-columns:auto minmax(0,1fr);min-height:40px;');
    expect(styles).toContain('.subagent-progress-card.collapsed .subagent-status-icon.interrupted + .subagent-progress-heading strong{opacity:.8}');
    expect(styles).toContain('.subagent-progress-node>details[open]>summary.completed .subagent-progress-goal strong{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}');
  });

  test('matches the folded Goal typography and keeps expanded status rows smaller than task descriptions', () => {
    const styles = css();
    expect(styles).toContain('.subagent-goal-preview{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12px;color:var(--text)}');
    expect(styles).toContain('.subagent-progress-heading strong{font-size:12px;font-weight:400}');
    expect(styles).toContain('.subagent-progress-goal strong{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:12px;font-weight:600}');
    expect(styles).toContain('.subagent-progress-goal small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--muted);font-size:10px;line-height:1.2;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}');
    expect(styles).toContain('.subagent-progress-detail-meta{margin:0;color:var(--muted);font-size:10px;line-height:1.2;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}');

  });

  test('reuses the exact main-chat transcript renderer without a subagent-only message renderer', () => {
    expect(app()).toContain("import { ChatTranscript");
    expect(app()).toContain('<ChatTranscript');
    expect(card()).toContain("import { ChatTranscript");
    expect(card()).toContain('showReasoning={showReasoning}');
    expect(card()).toContain('showToolCalls={showToolCalls}');
    expect(app()).toContain('showReasoning={props.showReasoning} showToolCalls={props.showToolCalls}');

    expect(transcript()).toContain('function ToolMessageView');
    expect(transcript()).toContain('buildTurnDetailItems');
    expect(transcript()).toContain('buildDesktopTurnBlocks');

    expect(css()).toContain('.subagent-progress-messages .msg-row.assistant .msg-body,.subagent-progress-messages .msg-row.user .msg-body{font-size:12px;line-height:1.45}');
  });

  test('shows a persisted goal separately while keeping the running subagent card visible', () => {
    const source = card();
    const styles = css();
    expect(source).toContain('const [goalExpanded, setGoalExpanded] = useState(false);');
    expect(source).toContain('setGoalExpanded(false)');
    expect(source).toContain('className="subagent-progress-stack"');
    expect(source).toContain('className="subagent-goal-panel" open={goalExpanded}');
    expect(source).toContain('<span className="subagent-status-icon subagent-goal-icon"><Target aria-hidden="true" /></span>');

    expect(source).toContain('const goal = visibleSnapshot.goal;');

    expect(source).toContain('<GoalMilestones goal={goal} />');
    expect(source).toContain("const milestones = [...goal.milestones].sort((left, right) => (right.timestamp || 0) - (left.timestamp || 0) || right.turn - left.turn);");
    expect(styles).toContain('.subagent-goal-milestones li p{margin:0;color:color-mix(in srgb,var(--text) 92%,var(--subagent-goal-accent));');
    expect(styles).toContain('.subagent-goal-body{max-height:min(62vh,720px);');

    expect(source).toContain('<SubagentTodoList todos={goal.todos} className="subagent-goal-todos" />');
    expect(source).toContain('<SubagentTodoList todos={node.todos} />');
    expect(source).toContain("function SubagentTodoList({ todos, className = '' }");

    expect(source).toContain('const statusCard = (visibleSnapshot.subagents.length > 0 || visibleSnapshot.error) && <section className={`subagent-progress-card');
    expect(styles).toContain('.subagent-goal-icon{color:var(--subagent-goal-accent);background:color-mix(in srgb,var(--subagent-goal-accent) 12%,transparent)}');
    expect(source).toContain('const goalMetadata = [');
    expect(source).toContain("tf('goals.turnProgress', goal.turnsUsed, goal.maxTurns)");
    expect(source).toContain('const goalElapsed = goalElapsedMinutes(goal, nowSeconds);');
    expect(source).toContain('goalElapsed >= 60');
    expect(source).toContain("tf('goals.elapsedHoursMinutes', Math.floor(goalElapsed / 60), goalElapsed % 60)");
    expect(source).toContain("tf('goals.elapsedMinutes', goalElapsed)");
    expect(source).toContain('className="subagent-goal-meta">{goalMetadata}</small>');
    expect(source).toContain('</div>\n    <footer className="subagent-goal-footer">{goalMetadata}</footer>');
    expect(source).toContain("const liveGoal = visibleSnapshot?.goal?.status === 'active';");
    expect(styles).toContain('.subagent-goal-copy{min-width:0;display:grid;gap:3px}');
    expect(styles).toContain('.subagent-goal-meta{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel[open] .subagent-goal-preview{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px}');
    expect(styles).toContain('.subagent-goal-body{max-height:min(62vh,720px);overflow-y:auto;overscroll-behavior:contain;padding:10px 12px;border-top:1px solid color-mix(in srgb,var(--border) 82%,transparent);font-size:14px;');
    expect(styles).toContain('.subagent-goal-todos li{font-size:12px}');
    expect(styles).toContain('.subagent-goal-milestones>header{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-bottom:8px;color:color-mix(in srgb,var(--text) 88%,var(--subagent-goal-accent));font-size:13px;');
    expect(styles).toContain('.subagent-goal-milestone-meta{display:flex;align-items:center;justify-content:space-between;gap:8px;color:color-mix(in srgb,var(--subagent-goal-accent) 82%,var(--text));font-size:11px;');
    expect(styles).toContain('.subagent-goal-milestone-meta time{font:500 11px/1.2 var(--mono)}');
    expect(styles).toContain('.subagent-goal-milestones li p{margin:0;color:color-mix(in srgb,var(--text) 92%,var(--subagent-goal-accent));font-size:13px;');
    expect(styles).toContain('.subagent-goal-footer{margin:0;padding:9px 12px;border-top:1px solid color-mix(in srgb,var(--subagent-goal-accent) 28%,var(--border));background:color-mix(in srgb,var(--subagent-goal-accent) 5%,transparent);color:color-mix(in srgb,var(--subagent-goal-accent) 78%,var(--text));font-size:11px;');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel[open] .subagent-goal-meta{display:block}');
    expect(i18n()).toContain("'goals.active': { en: 'Active', 'zh-CN': '进行中'");
    expect(i18n()).toContain("'goals.turnProgress': { en: '{0}/{1} turns', 'zh-CN': '{0}/{1} 轮'");
    expect(i18n()).toContain("'goals.elapsedMinutes': { en: '{0} min', 'zh-CN': '{0} 分钟'");
    expect(i18n()).toContain("'goals.elapsedHoursMinutes': { en: '{0} hr {1} min', 'zh-CN': '{0} 小时 {1} 分钟'");
    expect(source).toContain("if (!visibleSnapshot || visibleSnapshot.sessionId !== sessionId || (!visibleSnapshot.goal && !visibleSnapshot.subagents.length && !visibleSnapshot.error)) return null;");
    expect(source).toContain('socket.onmessage = (event) => {\n        if (stopped) return;');
    expect(source).toContain("aria-label={`${completed ? node.task : t('subagents.title')}: ${statusLabel(node.status)}`}");
    expect(source).toContain('export function SubagentProgressStack');
    expect(source.indexOf('{goal && <SubagentGoalPanel goal={goal}')).toBeLessThan(source.indexOf('{showCursorBar && <SubagentProgressCard'));
    expect(source).toContain('tone="cursor"');
    expect(source).toContain('tone="latest"');
    expect(source).toContain("<span className=\"subagent-progress-heading\"><strong>{selectedNode?.task || t('subagents.title')}</strong>");
    expect(styles).toContain('.subagent-progress-stack{width:100%;max-width:none;min-height:0;max-height:90%;display:flex;flex-direction:column;align-items:stretch;gap:0;pointer-events:none;');
    expect(styles).toContain('.subagent-goal-panel{--subagent-goal-accent:var(--accent-2);width:100%;flex:0 0 auto;pointer-events:auto;');
    expect(styles).toContain('color:var(--subagent-goal-accent)');
    expect(styles).toContain('.subagent-goal-panel[open] .subagent-goal-chevron{transform:rotate(90deg)}');
    expect(styles).toContain('.subagent-goal-body{');
    expect(styles).toContain('.subagent-goal-subgoals{margin:10px 0 0;padding:10px 0 0 20px;border-top:1px solid');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:52px}');
  });

  test('uses a responsive card that remains readable in compact desktop and mobile chat', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-card{');
    expect(styles).toContain('.subagent-progress-node>details>summary{');
    expect(styles).toContain('.subagent-progress-node>details[open]>summary .subagent-progress-goal strong{white-space:normal;overflow:visible;text-overflow:clip}');
    expect(styles).toContain('@media(max-width:760px){.subagent-progress-card');
    expect(styles).toContain('.desktop-compact-chat .subagent-progress-card');
  });

  test('keeps Goal and subagent header geometry fixed while their bodies toggle', () => {
    const source = card();
    const styles = css();
    expect(source).toContain('<span className="subagent-progress-mark"><span className={`subagent-status-icon ${node.status}`}>{statusIcon(node.status)}</span></span>');
    expect(styles).toContain('.subagent-progress-card .subagent-progress-panel-toggle.subagent-progress-header{grid-template-columns:32px minmax(0,1fr) 4ch 16px;min-height:56px;height:56px;max-height:56px;padding:8px 12px}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel .subagent-goal-summary{height:56px;min-height:56px;max-height:56px}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel .subagent-goal-summary{grid-template-columns:32px minmax(0,1fr) 16px;gap:10px;height:56px;min-height:56px;max-height:56px;padding:8px 12px}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel .subagent-goal-icon{width:32px;height:32px;display:grid;place-items:center;flex:0 0 32px}');
    expect(styles).toContain('.subagent-progress-card.expanded{padding-left:0;padding-right:0}');
    expect(styles).toContain('.subagent-progress-card.collapsed.completed-preview .subagent-progress-panel-toggle.subagent-progress-header{grid-template-columns:32px minmax(0,1fr) 4ch 16px;min-height:56px;height:56px;max-height:56px;padding:8px 12px}');
    expect(styles).toContain('.subagent-progress-stack>.subagent-goal-panel+.subagent-progress-card.expanded{padding-top:calc(var(--radius-card) + 2px)}');
    expect(styles).not.toContain('.subagent-goal-panel[open] .subagent-goal-preview{white-space:normal');
    expect(styles).not.toContain('.subagent-goal-panel[open] .subagent-goal-meta{display:none}');
  });

  test('fills the chat panel edge to edge with rounded outer card corners on desktop and mobile', () => {
    const styles = css();
    const cardRule = styles.match(/\.subagent-progress-card\{([^}]*)\}/)?.[1] || '';
    const overlayRule = styles.match(/\.subagent-progress-overlay\{([^}]*)\}/)?.[1] || '';
    expect(cardRule).toContain('width:100%');
    expect(cardRule).toContain('max-width:none');
    expect(cardRule).toContain('border-radius:var(--radius-card)');
    expect(overlayRule).toContain('padding:0');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:52px}.subagent-progress-overlay{padding:0}');

  });

  test('keeps shared transcript reasoning folds out of subagent node chrome', () => {
    const styles = css();
    expect(styles).toContain('.subagent-progress-node>details{');
    expect(styles).toContain('.subagent-progress-node>details>summary{');

    expect(styles).toContain('.msg-reasoning>summary{cursor:pointer;list-style:none;display:flex;align-items:center;gap:6px}');
  });

  test('uses the compact semantic radius for shared and subagent conversation details', () => {
    const styles = css();
    const subagentDetailRule = styles.match(/\.subagent-progress-node>details\{([^}]*)\}/)?.[1] || '';
    const sharedDetailRule = styles.match(/\.turn-detail-group\{([^}]*)\}/)?.[1] || '';
    expect(sharedDetailRule).toContain('border-radius:var(--radius-md)');
    expect(subagentDetailRule).toContain('border-radius:var(--radius-md)');
  });

  test('keeps a newly opened streaming detail at its latest content through the outer tree container', () => {
    const source = card();
    const styles = css();
    expect(source).toContain('const detailTreeRef = useRef<HTMLDivElement>(null);');
    expect(source).toContain('const followLatestDetailRef = useRef(true);');
    expect(source).toContain('tree.scrollTop = tree.scrollHeight;');
    expect(source).toContain('followLatestDetailRef.current = isSubagentDetailNearBottom(event.currentTarget);');
    expect(source).toContain('onDetailOpen={(runningNode) => { if (runningNode) startFollowingLatestDetail(); }}');
    expect(source).toContain('onDetailContentChange={followLatestDetail}');
    expect(source).toContain("if (open && node.status === 'running' && detailMessages.length > 0) onDetailContentChange();");

    expect(styles).toContain('.subagent-progress-panel-body .subagent-progress-tree{min-height:0;flex:1 1 auto;overflow-y:auto;overscroll-behavior:contain;touch-action:pan-y;-webkit-overflow-scrolling:touch;');

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
    expect(cardSource).toContain('const preview = previewSubagent(visibleSnapshot.subagents);');
    expect(cardSource).toContain('aria-expanded={expanded}');
    expect(cardSource).toContain("className={`subagent-progress-card subagent-card-${tone} ${expanded ? 'expanded' : 'collapsed'}${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}`}");
    expect(cardSource).toContain('{!expanded && preview && <SubagentProgressPreview');
    expect(cardSource).toContain('{expanded && <div className="subagent-progress-panel-body">');
    expect(styles).toContain('.subagent-progress-overlay{grid-row:2;grid-column:1;min-width:0;min-height:0;z-index:140;');
    expect(styles).toContain('.chat-main-panel .chat-scroll{grid-row:2;grid-column:1;');
    expect(styles).toContain('.chat-main-panel>.composer-wrap{grid-row:3;grid-column:1}');
    expect(styles).toContain('background:linear-gradient(145deg,color-mix(in srgb,var(--accent) 9%,var(--surface))');

    expect(styles).toContain('.subagent-progress-card.expanded{max-height:90%;');

    expect(styles).toContain('.subagent-progress-panel-body{min-height:0;');
    expect(styles).toContain('@media(max-width:760px){.subagent-goal-summary{min-height:52px}.subagent-progress-overlay{padding:0}');

  });
});
