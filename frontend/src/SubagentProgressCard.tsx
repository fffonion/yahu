import React, { useEffect, useMemo, useState } from 'react';
import { Bot, CheckCircle2, ChevronRight, Circle, LoaderCircle, XCircle } from 'lucide-react';
import { ChatTranscript, type ChatMessage } from './ChatTranscript';
import { t } from './i18n';
import {
  buildSubagentTree,
  formatSubagentElapsed,
  normalizeSubagentMessages,
  normalizeSubagentSnapshot,
  subagentElapsedSeconds,
  subagentMessagesUrl,
  subagentWebSocketUrl,
  type SubagentProgressSnapshot,
  type SubagentStatus,
  type SubagentTreeNode,
} from './subagentProgress';

export function SubagentProgressCard({ sessionId, showReasoning, showToolCalls, compact }: { sessionId: string; showReasoning: boolean; showToolCalls: boolean; compact: boolean }) {
  const [snapshot, setSnapshot] = useState<SubagentProgressSnapshot | null>(null);
  const [nowSeconds, setNowSeconds] = useState(() => Date.now() / 1000);

  useEffect(() => {
    setSnapshot(null);
    if (!sessionId || sessionId === '__webui_draft_session__') return;
    let socket: WebSocket | null = null;
    let reconnectTimer = 0;
    let stopped = false;
    let reconnectDelay = 750;

    const connect = () => {
      if (stopped) return;
      socket = new WebSocket(subagentWebSocketUrl(window.location, sessionId));
      socket.onopen = () => { reconnectDelay = 750; };
      socket.onmessage = (event) => {
        try {
          const next = normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId);
          if (next) setSnapshot(next);
        } catch {
          // Ignore malformed frames and keep the last valid projection.
        }
      };
      socket.onclose = () => {
        if (stopped) return;
        reconnectTimer = window.setTimeout(connect, reconnectDelay);
        reconnectDelay = Math.min(8_000, reconnectDelay * 2);
      };
    };

    connect();
    return () => {
      stopped = true;
      window.clearTimeout(reconnectTimer);
      socket?.close();
    };
  }, [sessionId]);

  const running = snapshot?.subagents.some((item) => item.status === 'running') || false;
  useEffect(() => {
    if (!running) return;
    setNowSeconds(Date.now() / 1000);
    const timer = window.setInterval(() => setNowSeconds(Date.now() / 1000), 1_000);
    return () => window.clearInterval(timer);
  }, [running]);

  const tree = useMemo(() => buildSubagentTree(snapshot?.subagents || [], sessionId), [snapshot?.subagents, sessionId]);
  if (!snapshot || (!snapshot.subagents.length && !snapshot.error)) return null;

  const finished = snapshot.subagents.filter((item) => item.status !== 'running').length;
  const total = snapshot.subagents.length;
  const completion = total ? Math.round((finished / total) * 100) : 0;
  return <section className="subagent-progress-card" aria-label={t('subagents.title')}>
    <header className="subagent-progress-header">
      <span className="subagent-progress-mark"><Bot aria-hidden="true" /></span>
      <span className="subagent-progress-heading"><strong>{t('subagents.title')}</strong><small>{running ? t('subagents.running') : t('subagents.finished')}</small></span>
      <span className="subagent-progress-count">{finished}/{total}</span>
    </header>
    <div className="subagent-progress-track" aria-hidden="true"><span style={{ width: `${completion}%` }} /></div>
    {snapshot.error && <p className="subagent-progress-error">{t('subagents.unavailable')}</p>}
    <div className="subagent-progress-tree">{tree.map((node) => <SubagentProgressNode key={node.sessionId} node={node} nowSeconds={nowSeconds} depth={0} showReasoning={showReasoning} showToolCalls={showToolCalls} compact={compact} />)}</div>
  </section>;
}

function SubagentProgressNode({ node, nowSeconds, depth, showReasoning, showToolCalls, compact }: { node: SubagentTreeNode; nowSeconds: number; depth: number; showReasoning: boolean; showToolCalls: boolean; compact: boolean }) {
  const elapsed = formatSubagentElapsed(subagentElapsedSeconds(node, nowSeconds));
  const [open, setOpen] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loadedMessageCount, setLoadedMessageCount] = useState(-1);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsError, setDetailsError] = useState(false);

  useEffect(() => {
    if (!open || loadedMessageCount === node.messageCount) return;
    const controller = new AbortController();
    setDetailsLoading(true);
    setDetailsError(false);
    fetch(subagentMessagesUrl(node.sessionId), { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        return normalizeSubagentMessages(await response.json());
      })
      .then((items) => {
        setMessages(items);
        setLoadedMessageCount(node.messageCount);
      })
      .catch((error) => {
        if (error?.name !== 'AbortError') setDetailsError(true);
      })
      .finally(() => {
        if (!controller.signal.aborted) setDetailsLoading(false);
      });
    return () => controller.abort();
  }, [loadedMessageCount, node.messageCount, node.sessionId, open]);

  return <div className={`subagent-progress-node depth-${Math.min(depth, 3)}`}>
    <details open={open} onToggle={(event) => setOpen(event.currentTarget.open)}>
      <summary>
        <span className={`subagent-status-icon ${node.status}`}>{statusIcon(node.status)}</span>
        <span className="subagent-progress-goal"><strong>{node.goal}</strong><small>{statusLabel(node.status)} · {elapsed}{node.currentTool ? ` · ${node.currentTool}` : ''}</small></span>
        <ChevronRight className="subagent-progress-chevron" aria-hidden="true" />
      </summary>
      <div className="subagent-progress-detail">
        <div className="subagent-progress-stats">
          {node.model && <span>{node.model}</span>}
          <span>{node.messageCount} {t('subagents.messages')}</span>
          <span>{node.toolCount} {t('subagents.tools')}</span>
          <span>{node.apiCalls} API</span>
        </div>
        {node.todos.length > 0 && <ul className="subagent-progress-todos">{node.todos.map((todo, index) => <li className={todo.status} key={`${todo.id}:${index}`}><span className="subagent-todo-box" aria-hidden="true">{todo.status === 'completed' ? '✓' : todo.status === 'in_progress' ? '–' : ''}</span><span>{todo.content}</span></li>)}</ul>}
        {node.activity.length > 0 && <p className="subagent-progress-activity"><span>{t('subagents.recent')}</span>{node.activity.map((item) => item.tool).join(' → ')}</p>}
        {detailsLoading && <p className="subagent-progress-detail-state">{t('subagents.loadingDetails')}</p>}
        {detailsError && <p className="subagent-progress-detail-state error">{t('subagents.detailsUnavailable')}</p>}
        {messages.length > 0 && <div className="subagent-progress-messages"><ChatTranscript messages={messages} showReasoning={showReasoning} showToolCalls={showToolCalls} assistantName={node.model || undefined} compact={compact} /></div>}
      </div>
    </details>
    {node.children.length > 0 && <div className="subagent-progress-children">{node.children.map((child) => <SubagentProgressNode key={child.sessionId} node={child} nowSeconds={nowSeconds} depth={depth + 1} showReasoning={showReasoning} showToolCalls={showToolCalls} compact={compact} />)}</div>}
  </div>;
}

function statusIcon(status: SubagentStatus) {
  if (status === 'running') return <LoaderCircle aria-hidden="true" />;
  if (status === 'completed') return <CheckCircle2 aria-hidden="true" />;
  if (status === 'failed' || status === 'timeout') return <XCircle aria-hidden="true" />;
  return <Circle aria-hidden="true" />;
}

function statusLabel(status: SubagentStatus): string {
  if (status === 'completed') return t('subagents.completed');
  if (status === 'failed') return t('subagents.failed');
  if (status === 'interrupted') return t('subagents.interrupted');
  if (status === 'timeout') return t('subagents.timeout');
  return t('subagents.running');
}
