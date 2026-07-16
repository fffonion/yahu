import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Bot, CheckCircle2, ChevronRight, Circle, LoaderCircle, Target, XCircle } from 'lucide-react';
import { ChatTranscript, type ChatMessage } from './ChatTranscript';
import { t, tf } from './i18n';
import {
  buildSubagentTree,
  createSubagentSnapshotGuard,
  formatSubagentElapsed,
  formatSubagentFinalMessages,
  isSubagentDetailNearBottom,
  mergeSubagentMessages,
  normalizeSubagentMessages,
  normalizeSubagentSnapshot,
  parseSubagentFinalStructuredContent,
  previewSubagent,
  subagentElapsedSeconds,
  subagentMessagesUrl,
  subagentSnapshotUrl,
  subagentWebSocketUrl,
  type PersistentGoal,
  type PersistentGoalStatus,
  type SubagentProgress,
  type SubagentProgressSnapshot,
  type SubagentStatus,
  type SubagentTodo,
  type SubagentTreeNode,
} from './subagentProgress';

type SubagentDetailCache = Record<string, { messages: ChatMessage[]; loadedMessageCount: number }>;
const EMPTY_CHAT_MESSAGES: ChatMessage[] = [];

export function SubagentProgressCard({ sessionId, beforeTime, showReasoning, showToolCalls, compact }: { sessionId: string; beforeTime: number | null | undefined; showReasoning: boolean; showToolCalls: boolean; compact: boolean }) {
  const [snapshot, setSnapshot] = useState<SubagentProgressSnapshot | null>(null);
  const [projectionPending, setProjectionPending] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [goalExpanded, setGoalExpanded] = useState(false);
  const [openNodeIds, setOpenNodeIds] = useState<Set<string>>(() => new Set());
  const [detailCache, setDetailCache] = useState<SubagentDetailCache>({});
  const [nowSeconds, setNowSeconds] = useState(() => Date.now() / 1000);
  const detailTreeRef = useRef<HTMLDivElement>(null);
  const followLatestDetailRef = useRef(true);

  const scrollToLatestDetail = useCallback((force: boolean) => {
    if (force) followLatestDetailRef.current = true;
    if (!followLatestDetailRef.current) return;
    window.requestAnimationFrame(() => {
      const tree = detailTreeRef.current;
      if (!tree || (!force && !followLatestDetailRef.current)) return;
      tree.scrollTop = tree.scrollHeight;
    });
  }, []);
  const startFollowingLatestDetail = useCallback(() => scrollToLatestDetail(true), [scrollToLatestDetail]);
  const followLatestDetail = useCallback(() => scrollToLatestDetail(false), [scrollToLatestDetail]);
  const setNodeOpen = useCallback((nodeSessionId: string, open: boolean) => {
    setOpenNodeIds((current) => {
      if (open) return new Set([nodeSessionId]);
      const next = new Set(current);
      next.delete(nodeSessionId);
      return next;
    });
  }, []);
  const cacheNodeMessages = useCallback((nodeSessionId: string, messageCount: number, items: ChatMessage[]) => {
    setDetailCache((current) => {
      const existing = current[nodeSessionId];
      const messages = mergeSubagentMessages(existing?.messages || [], items);
      if (existing && messages === existing.messages && existing.loadedMessageCount === messageCount) return current;
      return { ...current, [nodeSessionId]: { messages, loadedMessageCount: messageCount } };
    });
  }, []);

  useEffect(() => {
    setSnapshot(null);
    setProjectionPending(false);
    setExpanded(false);
    setGoalExpanded(false);
    setOpenNodeIds(new Set());
    setDetailCache({});
    followLatestDetailRef.current = true;
  }, [sessionId]);

  useEffect(() => {
    if (!sessionId || sessionId === '__webui_draft_session__') return;
    let stopped = false;
    if (beforeTime === null) {
      setProjectionPending(false);
      return;
    }
    if (typeof beforeTime === 'number') {
      const controller = new AbortController();
      const requestGuard = createSubagentSnapshotGuard();
      setProjectionPending(true);
      setSnapshot((current) => current || {
        type: 'subagents.snapshot',
        sessionId,
        generatedAt: Date.now() / 1000,
        subagents: [],
      });
      fetch(subagentSnapshotUrl(sessionId, beforeTime), { signal: controller.signal })
        .then((response) => {
          if (!response.ok) throw new Error(`subagent snapshot failed (${response.status})`);
          return response.json();
        })
        .then((payload) => {
          if (!requestGuard.isActive(controller.signal)) return;
          const next = normalizeSubagentSnapshot(payload, sessionId);
          if (next) setSnapshot(next);
          setProjectionPending(false);
        })
        .catch((error) => {
          if (!requestGuard.isActive(controller.signal)) return;
          setProjectionPending(false);
          setSnapshot((current) => current ? { ...current, error: String(error) } : {
            type: 'subagents.snapshot',
            sessionId,
            generatedAt: Date.now() / 1000,
            subagents: [],
            error: String(error),
          });
        });
      return () => { requestGuard.stop(); controller.abort(); };
    }
    let socket: WebSocket | null = null;
    let reconnectTimer = 0;
    let reconnectDelay = 750;
    setProjectionPending(true);
    setSnapshot((current) => current ? { ...current, error: undefined } : {
      type: 'subagents.snapshot',
      sessionId,
      generatedAt: Date.now() / 1000,
      subagents: [],
    });

    const connect = () => {
      if (stopped) return;
      socket = new WebSocket(subagentWebSocketUrl(window.location, sessionId));
      socket.onopen = () => { reconnectDelay = 750; };
      socket.onmessage = (event) => {
        if (stopped) return;
        try {
          const next = normalizeSubagentSnapshot(JSON.parse(String(event.data)), sessionId);
          if (next) {
            setSnapshot(next);
            setProjectionPending(false);
          }
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
  }, [beforeTime, sessionId]);

  const runningCount = snapshot?.subagents.filter((item) => item.status === 'running').length || 0;
  const running = runningCount > 0;
  useEffect(() => {
    if (!running) return;
    setNowSeconds(Date.now() / 1000);
    const timer = window.setInterval(() => setNowSeconds(Date.now() / 1000), 1_000);
    return () => window.clearInterval(timer);
  }, [running]);

  const tree = useMemo(() => buildSubagentTree(snapshot?.subagents || [], sessionId), [snapshot?.subagents, sessionId]);
  if (!snapshot || snapshot.sessionId !== sessionId || (!snapshot.goal && !snapshot.subagents.length && !snapshot.error && !projectionPending)) return null;

  const goal = snapshot.goal;
  const preview = previewSubagent(snapshot.subagents);
  const completedGoalTodos = goal?.todos.filter((item) => item.status === 'completed').length || 0;
  const goalMetadata = goal ? [
    persistentGoalStatusLabel(goal.status),
    tf('goals.turnProgress', goal.turnsUsed, goal.maxTurns),
    goal.todos.length ? tf('subagents.todoProgress', completedGoalTodos, goal.todos.length) : '',
  ].filter(Boolean).join(' · ') : '';
  const finished = snapshot.subagents.filter((item) => item.status !== 'running').length;
  const total = snapshot.subagents.length;
  const completion = total ? Math.round((finished / total) * 100) : 0;
  return <div className="subagent-progress-stack">
    {goal && <details className="subagent-goal-panel" open={goalExpanded}>
      <summary className="subagent-goal-summary" aria-label={t('goals.title')} onClick={(event) => { event.preventDefault(); setGoalExpanded((open) => !open); }}>
        <span className="subagent-status-icon subagent-goal-icon"><Target aria-hidden="true" /></span>
        <span className="subagent-goal-copy">
          <span className="subagent-goal-preview">{goal.text}</span>
          <small className="subagent-goal-meta">{goalMetadata}</small>
        </span>
        <ChevronRight className="subagent-goal-chevron" aria-hidden="true" />
      </summary>
      <div className="subagent-goal-body">
        <SubagentTodoList todos={goal.todos} className="subagent-goal-todos" />
        {goal.subgoals.length > 0 && <ul className="subagent-goal-subgoals">{goal.subgoals.map((item, index) => <li key={index}>{item}</li>)}</ul>}
        <GoalMilestones goal={goal} />
      </div>
      <footer className="subagent-goal-footer">{goalMetadata}</footer>
    </details>}
    {(snapshot.subagents.length > 0 || snapshot.error || projectionPending) && <section className={`subagent-progress-card ${expanded ? 'expanded' : 'collapsed'}${!expanded && preview?.status === 'completed' ? ' completed-preview' : ''}`} aria-label={t('subagents.title')}>
    <button type="button" className="subagent-progress-panel-toggle subagent-progress-header" aria-expanded={expanded} onClick={() => setExpanded((value) => !value)}>
      {!expanded && preview && <SubagentProgressPreview node={preview} runningCount={runningCount} nowSeconds={nowSeconds} />}
      {(expanded || !preview) && <>
        <span className="subagent-progress-mark"><Bot aria-hidden="true" /></span>
        <span className="subagent-progress-heading"><strong>{t('subagents.title')}</strong><small>{projectionPending ? t('subagents.refreshing') : running ? t('subagents.running') : t('subagents.finished')}</small></span>
        {!projectionPending && <span className="subagent-progress-count">{finished}/{total}</span>}
        <ChevronRight className="subagent-progress-panel-chevron" aria-hidden="true" />
      </>}
    </button>
    {expanded && <div className="subagent-progress-panel-body">
      <div className="subagent-progress-track" aria-hidden="true"><span style={{ width: `${completion}%` }} /></div>
      {snapshot.error && <p className="subagent-progress-error">{t('subagents.unavailable')}</p>}
      <div className="subagent-progress-tree" ref={detailTreeRef} onScroll={(event) => { followLatestDetailRef.current = isSubagentDetailNearBottom(event.currentTarget); }}>{tree.map((node) => <SubagentProgressNode key={node.sessionId} node={node} openNodeIds={openNodeIds} onOpenChange={setNodeOpen} detailCache={detailCache} onMessagesLoaded={cacheNodeMessages} nowSeconds={nowSeconds} depth={0} showReasoning={showReasoning} showToolCalls={showToolCalls} compact={compact} onDetailOpen={startFollowingLatestDetail} onDetailContentChange={followLatestDetail} />)}</div>
    </div>}
    </section>}
  </div>;
}

export function GoalMilestones({ goal }: { goal: PersistentGoal }) {
  const milestones = [...goal.milestones].sort((left, right) => (right.timestamp || 0) - (left.timestamp || 0) || right.turn - left.turn);
  if (goal.pausedReason && !milestones.some((item) => item.reason === goal.pausedReason)) {
    milestones.unshift({ turn: goal.turnsUsed, verdict: 'paused', reason: goal.pausedReason });
  }
  if (milestones.length === 0 && goal.lastReason) {
    milestones.push({ turn: goal.turnsUsed, verdict: goal.status, reason: goal.lastReason });
  }
  if (milestones.length === 0) return null;
  return <section className="subagent-goal-milestones">
    <header><span>{t('goals.milestones')}</span><small>{milestones.length}</small></header>
    <ol>{milestones.map((item, index) => {
      const date = item.timestamp ? new Date(item.timestamp * 1000) : null;
      return <li key={`${item.turn}:${item.timestamp || 0}:${index}`} data-verdict={item.verdict}>
        <div className="subagent-goal-milestone-meta">
          <span>{tf('goals.round', item.turn)}</span>
          {date && <time dateTime={date.toISOString()}>{date.toLocaleString([], { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' })}</time>}
        </div>
        <p>{item.reason}</p>
      </li>;
    })}</ol>
  </section>;
}

function SubagentProgressPreview({ node, runningCount, nowSeconds }: { node: SubagentProgress; runningCount: number; nowSeconds: number }) {
  const elapsed = formatSubagentElapsed(subagentElapsedSeconds(node, nowSeconds));
  const completed = node.status === 'completed';
  const previewStatus = runningCount > 1 ? tf('subagents.runningCount', runningCount) : statusLabel(node.status);
  return <>
    <span className={`subagent-status-icon ${node.status}`}>{statusIcon(node.status)}</span>
    <span className="subagent-progress-heading" aria-label={`${completed ? node.task : t('subagents.title')}: ${statusLabel(node.status)}`}><strong>{completed ? node.task : t('subagents.title')}</strong>{!completed && <small>{previewStatus} · {elapsed}{node.currentTool ? ` · ${node.currentTool}` : ''}</small>}</span>
    {!completed && <ChevronRight className="subagent-progress-panel-chevron" aria-hidden="true" />}
  </>;
}

export function SubagentProgressNode({ node, openNodeIds, onOpenChange, detailCache, onMessagesLoaded, nowSeconds, depth, showReasoning, showToolCalls, compact, onDetailOpen, onDetailContentChange }: { node: SubagentTreeNode; openNodeIds: ReadonlySet<string>; onOpenChange: (sessionId: string, open: boolean) => void; detailCache: Readonly<SubagentDetailCache>; onMessagesLoaded: (sessionId: string, messageCount: number, messages: ChatMessage[]) => void; nowSeconds: number; depth: number; showReasoning: boolean; showToolCalls: boolean; compact: boolean; onDetailOpen: () => void; onDetailContentChange: () => void }) {
  const elapsed = formatSubagentElapsed(subagentElapsedSeconds(node, nowSeconds));
  const completed = node.status === 'completed';
  const open = openNodeIds.has(node.sessionId);
  const cachedDetail = detailCache[node.sessionId];
  const messages = cachedDetail?.messages || EMPTY_CHAT_MESSAGES;
  const loadedMessageCount = cachedDetail?.loadedMessageCount ?? -1;
  const [detailsLoading, setDetailsLoading] = useState(false);
  const [detailsError, setDetailsError] = useState(false);
  const detailMessages = useMemo(() => {
    const formatted = formatSubagentFinalMessages(messages);
    if (node.status === 'running' || !node.summary) return formatted;
    const lastContentMessage = [...formatted].reverse().find((message) => message.content.trim());
    if (lastContentMessage?.role === 'assistant') return formatted;
    return [...formatted, {
      id: `${node.sessionId}:summary`,
      role: 'assistant' as const,
      content: node.summary,
      structuredContent: parseSubagentFinalStructuredContent(node.summary),
      timestamp: node.endedAt,
    }];
  }, [messages, node.endedAt, node.sessionId, node.status, node.summary]);

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
        if (!controller.signal.aborted) onMessagesLoaded(node.sessionId, node.messageCount, items);
      })
      .catch((error) => {
        if (error?.name !== 'AbortError') setDetailsError(true);
      })
      .finally(() => {
        if (!controller.signal.aborted) setDetailsLoading(false);
      });
    return () => controller.abort();
  }, [loadedMessageCount, node.messageCount, node.sessionId, onMessagesLoaded, open]);

  useLayoutEffect(() => {
    if (open && detailMessages.length > 0) onDetailContentChange();
  }, [detailMessages, onDetailContentChange, open]);

  return <div className={`subagent-progress-node depth-${Math.min(depth, 3)}`}>
    <details open={open}>
      <summary className={completed ? 'completed' : undefined} onClick={(event) => { event.preventDefault(); const nextOpen = !open; onOpenChange(node.sessionId, nextOpen); if (nextOpen) onDetailOpen(); }}>
        <span className={`subagent-status-icon ${node.status}`}>{statusIcon(node.status)}</span>
        <span className="subagent-progress-goal"><strong>{node.task}{node.ancestryOmitted && <span className="subagent-progress-omitted-ancestry" title={t('subagents.parentOmitted')}> · {t('subagents.parentOmitted')}</span>}</strong>{!completed && <small>{statusLabel(node.status)} · {elapsed}{node.currentTool ? ` · ${node.currentTool}` : ''}</small>}</span>
        {!completed && <ChevronRight className="subagent-progress-chevron" aria-hidden="true" />}
      </summary>
      <div className="subagent-progress-detail">
        {completed && <p className="subagent-progress-detail-meta">{statusLabel(node.status)} · {elapsed}</p>}
        <SubagentTodoList todos={node.todos} />
        {detailsLoading && <p className="subagent-progress-detail-state">{t('subagents.loadingDetails')}</p>}
        {detailsError && <p className="subagent-progress-detail-state error">{t('subagents.detailsUnavailable')}</p>}
        {detailMessages.length > 0 && <div className="subagent-progress-messages"><ChatTranscript messages={detailMessages} showReasoning={showReasoning} showToolCalls={showToolCalls} compact={compact} /></div>}
      </div>
    </details>
    {node.children.length > 0 && <div className="subagent-progress-children">{node.children.map((child) => <SubagentProgressNode key={child.sessionId} node={child} openNodeIds={openNodeIds} onOpenChange={onOpenChange} detailCache={detailCache} onMessagesLoaded={onMessagesLoaded} nowSeconds={nowSeconds} depth={depth + 1} showReasoning={showReasoning} showToolCalls={showToolCalls} compact={compact} onDetailOpen={onDetailOpen} onDetailContentChange={onDetailContentChange} />)}</div>}
  </div>;
}

function SubagentTodoList({ todos, className = '' }: { todos: SubagentTodo[]; className?: string }) {
  if (!todos.length) return null;
  return <ul className={`subagent-progress-todos${className ? ` ${className}` : ''}`}>{todos.map((todo, index) => <li className={todo.status} key={`${todo.id}:${index}`}><span className="subagent-todo-box" aria-hidden="true">{todo.status === 'completed' ? '✓' : todo.status === 'in_progress' ? '–' : ''}</span><span>{todo.content}</span></li>)}</ul>;
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

function persistentGoalStatusLabel(status: PersistentGoalStatus): string {
  if (status === 'paused') return t('goals.paused');
  if (status === 'done') return t('goals.done');
  return t('goals.active');
}
