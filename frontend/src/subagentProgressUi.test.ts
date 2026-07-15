import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const card = () => readFileSync(new URL('./SubagentProgressCard.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const transcript = () => {
  const path = new URL('./ChatTranscript.tsx', import.meta.url);
  return existsSync(path) ? readFileSync(path, 'utf8') : '';
};

describe('subagent progress UI', () => {
  test('renders the websocket-backed progress card inside the chat stream', () => {
    expect(app()).toContain("import { SubagentProgressCard } from './SubagentProgressCard';");
    expect(app()).toContain('<SubagentProgressCard sessionId={props.activeSessionId} showReasoning={props.showReasoning} showToolCalls={props.showToolCalls} compact={props.desktopCompactMessages} />');
    expect(card()).toContain('new WebSocket(subagentWebSocketUrl(window.location, sessionId))');
    expect(card()).toContain('normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId)');
  });

  test('keeps todos and shared conversation detail while removing counters, model, and recent activity chrome', () => {
    const source = card();
    expect(source).toContain('className="subagent-progress-tree"');
    expect(source).toContain('className="subagent-progress-todos"');
    expect(source).toContain('className="subagent-progress-messages"');
    expect(source).toContain('formatSubagentFinalMessages(messages)');
    expect(source).toContain('structuredContent: parseSubagentFinalStructuredContent(node.summary)');
    expect(source).not.toContain('prettyFormatSubagentFinalMessage');
    expect(source).toContain('<SubagentProgressNode key={child.sessionId}');
    expect(source).not.toContain('className="subagent-progress-stats"');
    expect(source).not.toContain('className="subagent-progress-activity"');
    expect(source).not.toContain('assistantName={node.model');
  });

  test('lazy-loads full conversation detail and preserves expanded session ids across full snapshot replacements', () => {
    const source = card();
    expect(source).toContain("const [openNodeIds, setOpenNodeIds] = useState<Set<string>>(() => new Set());");
    expect(source).toContain('openNodeIds={openNodeIds}');
    expect(source).toContain('const open = openNodeIds.has(node.sessionId);');
    expect(source).toContain('onOpenChange={setNodeOpen}');
    expect(source).toContain('setOpenNodeIds((current) => {');
    expect(source).toContain('setOpenNodeIds(new Set());');
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
    expect(styles).toContain('@media(max-width:760px){.subagent-progress-overlay{padding:0}');
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

  test('keeps a newly opened streaming detail at its latest content until the user scrolls upward', () => {
    const source = card();
    expect(source).toContain('const detailTreeRef = useRef<HTMLDivElement>(null);');
    expect(source).toContain('const followLatestDetailRef = useRef(true);');
    expect(source).toContain('tree.scrollTop = tree.scrollHeight;');
    expect(source).toContain('followLatestDetailRef.current = isSubagentDetailNearBottom(event.currentTarget);');
    expect(source).toContain('onDetailOpen={startFollowingLatestDetail}');
    expect(source).toContain('onDetailContentChange={followLatestDetail}');
    expect(source).toContain('if (open && detailMessages.length > 0) onDetailContentChange();');
    expect(css()).toContain('overflow-y:auto;overscroll-behavior:contain;touch-action:pan-y;-webkit-overflow-scrolling:touch;');
  });

  test('floats above chat history, previews only the latest subagent while collapsed, and expands to ninety percent height', () => {
    const appSource = app();
    const cardSource = card();
    const styles = css();
    const overlayIndex = appSource.indexOf('className="subagent-progress-overlay"');
    const chatScrollIndex = appSource.indexOf('className="chat-scroll"');
    expect(overlayIndex).toBeGreaterThan(-1);
    expect(chatScrollIndex).toBeGreaterThan(overlayIndex);
    expect(cardSource).toContain('const [expanded, setExpanded] = useState(false);');
    expect(cardSource).toContain('const latest = latestSubagent(snapshot.subagents);');
    expect(cardSource).toContain('aria-expanded={expanded}');
    expect(cardSource).toContain("className={`subagent-progress-card ${expanded ? 'expanded' : 'collapsed'}${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}`}");
    expect(cardSource).toContain('{!expanded && preview && <SubagentProgressPreview');
    expect(cardSource).toContain('{expanded && <div className="subagent-progress-panel-body">');
    expect(styles).toContain('.subagent-progress-overlay{grid-row:2;grid-column:1;min-width:0;min-height:0;z-index:140;');
    expect(styles).toContain('.chat-main-panel .chat-scroll{grid-row:2;grid-column:1;');
    expect(styles).toContain('.chat-main-panel>.composer-wrap{grid-row:3;grid-column:1}');
    expect(styles).toContain('background:linear-gradient(145deg,color-mix(in srgb,var(--accent) 9%,var(--surface))');
    expect(styles).not.toContain('background:linear-gradient(145deg,color-mix(in srgb,var(--accent-soft) 46%,var(--surface))');
    expect(styles).toContain('.subagent-progress-card.expanded{height:90%;max-height:90%;');
    expect(styles).toContain('.subagent-progress-panel-body{min-height:0;');
    expect(styles).toContain('@media(max-width:760px){.subagent-progress-overlay{padding:0}');
  });
});
