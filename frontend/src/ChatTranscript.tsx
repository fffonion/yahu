import React, { useMemo, useState } from 'react';
import {
  Brain,
  CalendarClock,
  CheckSquare,
  ChevronRight,
  CircleHelp,
  Code,
  Eye,
  FileText,
  Globe,
  History,
  Home,
  Image as ImageIcon,
  Info,
  Layout,
  MessageSquare,
  Network,
  Puzzle,
  Repeat,
  Search,
  Send,
  Server,
  Settings,
  Terminal,
  Users,
  Video,
  Volume2,
} from 'lucide-react';
import { findNewMessageSplitIndex } from './chatNewMessages';
import { t, tf } from './i18n';
import { markdownText } from './markdown';
import { isAssistantToolPreludeMessage, isToolLikeMessage, visibleChatMessages } from './messageVisibility';
import { parseSessionStateMessage, type SessionTaskStatus } from './sessionStateMessage';
import { formatChatMessageTime } from './sessionTime';
import { summarizeToolMessage } from './toolMessage';
import {
  buildDesktopTurnBlocks,
  buildTurnDetailItems,
  type SessionStateMessageItem,
  type SpecialContextGroupItem,
  type TurnDetailBlock,
  type TurnDetailGroupItem,
  type TurnDetailMetadata,
} from './turnDetails';

export type Role = 'user' | 'assistant' | 'system' | 'tool';
export type ChatTurnMetrics = { elapsedMs?: number; inputTokens?: number; outputTokens?: number; totalTokens?: number; costUsd?: number };
export type ChatMessage = { id: string; role: Role; content: string; structuredContent?: { value: unknown }; reasoning?: string; timestamp?: string | number; pending?: boolean; toolName?: string; toolInput?: unknown; toolCalls?: unknown; toolCallId?: string; tokenCount?: number; turnMetrics?: ChatTurnMetrics; turnDetails?: TurnDetailMetadata; historyGap?: { after: number; before: number }; model?: string; provider?: string; platformSenderName?: string; platformSenderId?: string };

type LoadTurnDetails = (detail: TurnDetailMetadata) => Promise<ChatMessage[]>;

type ChatTranscriptProps = {
  messages: ChatMessage[];
  showReasoning: boolean;
  showToolCalls: boolean;
  assistantName?: string;
  compact?: boolean;
  newMessageBoundaryId?: string | null;
  loadTurnDetails?: LoadTurnDetails;
};

export function ChatTranscript({
  messages,
  showReasoning,
  showToolCalls,
  assistantName,
  compact = false,
  newMessageBoundaryId,
  loadTurnDetails,
}: ChatTranscriptProps) {
  const visibleMessages = useMemo(
    () => visibleChatMessages<ChatMessage>(messages, showReasoning, showToolCalls),
    [messages, showReasoning, showToolCalls],
  );
  const turnDetailItems = useMemo(() => buildTurnDetailItems(visibleMessages), [visibleMessages]);
  const desktopTurnBlocks = useMemo(() => buildDesktopTurnBlocks(turnDetailItems), [turnDetailItems]);
  const splitIdx = findNewMessageSplitIndex(visibleMessages, newMessageBoundaryId || undefined);

  if (compact) {
    return <>{desktopTurnBlocks.map((block) => {
      const showSplit = splitIdx >= 0 && block.sourceIndexes.includes(splitIdx);
      return <React.Fragment key={block.id}>
        {showSplit && <NewMessagesSeparator />}
        <DesktopTurnBlock block={block} showReasoning={showReasoning} assistantName={assistantName} loadTurnDetails={loadTurnDetails} />
      </React.Fragment>;
    })}</>;
  }

  return <>{turnDetailItems.map((item) => {
    const sourceIndex = item.sourceIndexes[0] ?? -1;
    const showSplit = splitIdx >= 0 && item.sourceIndexes.includes(splitIdx);
    let rendered: React.ReactNode;
    if (item.kind === 'detailGroup') {
      rendered = <TurnDetailGroup item={item} showReasoning={showReasoning} assistantName={assistantName} loadTurnDetails={loadTurnDetails} />;
    } else if (item.kind === 'specialContextGroup') {
      rendered = <SpecialContextGroup item={item} />;
    } else if (item.kind === 'sessionState') {
      rendered = <SessionStateMessage item={item} />;
    } else {
      rendered = <MessageView message={item.message} showReasoning={showReasoning} assistantName={assistantName} />;
    }
    return <React.Fragment key={item.kind === 'message' ? item.message.id || sourceIndex : item.id}>
      {showSplit && <NewMessagesSeparator />}
      {rendered}
    </React.Fragment>;
  })}</>;
}

function NewMessagesSeparator() {
  return <div className="new-messages-separator" role="separator"><span className="new-messages-label">{t('chat.newMessages')}</span></div>;
}

export function StructuredDataView({ value }: { value: unknown }) {
  if (value === null || value === undefined) return <span className="tool-empty">null</span>;
  if (Array.isArray(value)) return <div className="tool-children">{value.map((item, index) => <div className="tool-field" key={index}><span className="tool-key">{index}</span><StructuredDataView value={item} /></div>)}</div>;
  if (typeof value === 'object') {
    return <div className="tool-children">{Object.entries(value as Record<string, unknown>).map(([key, child]) => <div className="tool-field" key={key}><span className="tool-key">{key}</span><StructuredDataView value={child} /></div>)}</div>;
  }
  return <span className={`tool-scalar ${typeof value}`}>{String(value)}</span>;
}

function ToolDetailSection({ title, value }: { title: string; value: unknown }) {
  return <section className="tool-detail-section"><h4>{title}</h4><StructuredDataView value={value} /></section>;
}

function getToolIcon(toolName: string): React.ReactNode {
  const name = (toolName || '').toLowerCase().replace(/^functions\./, '');
  if (name.startsWith('browser_')) return <Globe />;
  if (name === 'terminal' || name === 'process') return <Terminal />;
  if (name === 'read_file' || name === 'write_file' || name === 'patch') return <FileText />;
  if (name === 'search_files') return <Search />;
  if (name === 'execute_code') return <Code />;
  if (name === 'web_search' || name === 'x_search') return <Search />;
  if (name === 'web_extract') return <Globe />;
  if (name === 'vision_analyze') return <Eye />;
  if (name === 'image_generate') return <ImageIcon />;
  if (name === 'video_generate' || name === 'video_analyze') return <Video />;
  if (name === 'text_to_speech') return <Volume2 />;
  if (name.startsWith('skill')) return <Puzzle />;
  if (name === 'memory') return <Brain />;
  if (name === 'session_search') return <History />;
  if (name === 'delegate_task') return <Users />;
  if (name === 'cronjob') return <CalendarClock />;
  if (name === 'clarify') return <CircleHelp />;
  if (name === 'send_message') return <Send />;
  if (name === 'todo') return <CheckSquare />;
  if (name.startsWith('kanban_')) return <Layout />;
  if (name.startsWith('ha_')) return <Home />;
  if (name === 'discord' || name === 'discord_admin') return <MessageSquare />;
  if (name.startsWith('feishu_')) return <FileText />;
  if (name.startsWith('yb_')) return <MessageSquare />;
  if (name.startsWith('mcp_')) return <Server />;
  if (name === 'workflow_run') return <Repeat />;
  if (name === 'mixture_of_agents') return <Network />;
  if (name === 'computer_use') return <Globe />;
  return <Settings />;
}

function ToolMessageView({ message, suppressMessageAnchor = false }: { message: ChatMessage; suppressMessageAnchor?: boolean }) {
  const [expanded, setExpanded] = useState(false);
  const summary = useMemo(
    () => summarizeToolMessage(message.content, message.toolName, message.toolInput),
    [message.content, message.toolName, message.toolInput],
  );
  const isError = summary.status !== 'ok';
  const toolName = summary.toolName;
  return <article className={`msg-row tool${isError ? ' tool-error' : ''}`} data-message-id={!suppressMessageAnchor ? message.id || undefined : undefined}>
    <div className="msg-content tool-card">
      <button type="button" className="tool-summary" aria-expanded={expanded} onClick={() => setExpanded((value) => !value)}>
        <span className="tool-inline-icon">{getToolIcon(toolName)}</span>
        <span className={`tool-title${isError ? ' err' : ''}`}>{summary.title}</span>
        <span className="tool-subtitle">{summary.subtitle}</span>
        <ChevronRight className={`tool-chevron ${expanded ? 'open' : ''}`} />
      </button>
      {expanded && <div className="tool-detail">
        {summary.input !== undefined && <ToolDetailSection title={t('tool.invocation')} value={summary.input} />}
        <ToolDetailSection title={t('tool.result')} value={summary.result} />
      </div>}
    </div>
  </article>;
}

function roleName(role: Role) { return role === 'assistant' ? 'Hermes Agent' : role === 'tool' ? 'Tool' : role === 'system' ? 'System' : 'You'; }
function messageRoleName(message: ChatMessage, assistantName?: string) { return message.role === 'assistant' ? (message.model || assistantName || 'Hermes Agent') : roleName(message.role); }
function messageSenderLabel(message: ChatMessage, assistantName?: string) {
  if (message.role === 'user' && message.platformSenderName) return { name: message.platformSenderName, id: message.platformSenderId };
  return { name: messageRoleName(message, assistantName) };
}

function formatTurnDuration(ms?: number): string {
  if (!Number.isFinite(ms || 0) || !ms || ms <= 0) return 'time —';
  if (ms < 1000) return 'time <1s';
  const seconds = ms / 1000;
  if (seconds < 60) return `time ${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `time ${minutes}m ${rest}s`;
}
function formatTurnTokenCount(value: number): string {
  return `tokens ${Math.round(value).toLocaleString()}`;
}
function formatTurnCost(value: number): string {
  if (value < 0.0001) return `cost $${value.toFixed(6)}`;
  if (value < 0.01) return `cost $${value.toFixed(4)}`;
  return `cost $${value.toFixed(3)}`;
}
function messageTurnMetadata(message: ChatMessage): string {
  if (message.role !== 'assistant' || message.pending || isAssistantToolPreludeMessage(message) || !message.content.trim()) return '';
  const metrics = message.turnMetrics || {};
  const elapsedMs = metrics.elapsedMs;
  const totalTokens = metrics.totalTokens ?? message.tokenCount ?? ((metrics.inputTokens || metrics.outputTokens) ? (metrics.inputTokens || 0) + (metrics.outputTokens || 0) : undefined);
  const detail = metrics.inputTokens || metrics.outputTokens ? ` (in ${Math.round(metrics.inputTokens || 0).toLocaleString()} / out ${Math.round(metrics.outputTokens || 0).toLocaleString()})` : '';
  const metadataParts = [formatTurnDuration(elapsedMs)];
  if (totalTokens !== undefined) metadataParts.push(`${formatTurnTokenCount(totalTokens)}${detail}`);
  if (metrics.costUsd !== undefined) metadataParts.push(formatTurnCost(metrics.costUsd));
  return metadataParts.join(' · ');
}
function formatReasoningDuration(ms?: number): string {
  const formatted = formatTurnDuration(ms);
  return formatted === 'time —' ? '' : formatted.replace(/^time\s+/, '');
}
function reasoningSummaryLabel(message: ChatMessage): string {
  const elapsed = formatReasoningDuration(message.turnMetrics?.elapsedMs);
  return elapsed ? `${t('chat.reasoned')} ${elapsed}` : t('chat.reasoned');
}

function HistoryCoverageGap({ message }: { message: ChatMessage }) {
  return <article className="history-coverage-gap" role="separator" data-message-id={message.id || undefined}>
    <span className="history-coverage-gap-line" aria-hidden="true" />
    <span className="history-coverage-gap-label"><Info aria-hidden="true" />{t('chat.historyCoverageGap')}</span>
    <span className="history-coverage-gap-line" aria-hidden="true" />
  </article>;
}

function MessageView({ message, showReasoning = false, assistantName, suppressMessageAnchor = false }: { message: ChatMessage; showReasoning?: boolean; assistantName?: string; suppressMessageAnchor?: boolean }) {
  if (message.historyGap) return <HistoryCoverageGap message={message} />;
  if (isToolLikeMessage(message)) return <ToolMessageView message={message} suppressMessageAnchor={suppressMessageAnchor} />;
  const isPending = !!message.pending;
  const isToolPrelude = isAssistantToolPreludeMessage(message);
  const senderLabel = messageSenderLabel(message, assistantName);
  const turnMetadata = messageTurnMetadata(message);
  const reasoningSummary = reasoningSummaryLabel(message);
  const showTurnMetadata = message.role === 'assistant' && !isPending && !isToolPrelude && !!turnMetadata;
  return <article className={`msg-row ${message.role}${isPending ? ' pending' : ''}${isToolPrelude ? ' tool-prelude' : ''}`} data-message-id={!suppressMessageAnchor ? message.id || undefined : undefined}>
    <div className="msg-content">
      <div className="msg-meta">
        <span className="msg-sender-name">{senderLabel.name}{senderLabel.id && <small className="msg-sender-id">{senderLabel.id}</small>}</span>
        <time>{formatChatMessageTime(message.timestamp)}</time>
        {isPending && <span className="stream-state" aria-label={t('chat.streaming')}><span className="stream-dots"><i /><i /><i /></span><span className="stream-label">{t('chat.streaming')}</span></span>}
      </div>
      {message.reasoning && showReasoning && <details className="msg-reasoning msg-reasoning-collapsed" aria-label={reasoningSummary}><summary><span className="reasoning-chevron"><ChevronRight /></span> <span>{reasoningSummary}</span></summary><pre>{message.reasoning}</pre></details>}
      <div className="msg-body">
        {message.structuredContent ? <StructuredDataView value={message.structuredContent.value} /> : <div className="md-content" dangerouslySetInnerHTML={{ __html: markdownText(message.content || (isPending ? '…' : '')) }} />}
        {isPending && <span className="stream-caret" aria-hidden="true" />}
      </div>
      {showTurnMetadata && <div className="msg-turn-metadata" aria-label={t('chat.details')}>{turnMetadata}</div>}
    </div>
  </article>;
}

function TurnDetailGroup({ item, showReasoning, assistantName, loadTurnDetails }: { item: TurnDetailGroupItem<ChatMessage>; showReasoning: boolean; assistantName?: string; loadTurnDetails?: LoadTurnDetails }) {
  const [open, setOpen] = useState(() => !!item.defaultOpen);
  const [loadedMessages, setLoadedMessages] = useState<ChatMessage[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const detailMessages = useMemo(() => loadedMessages.length ? visibleChatMessages<ChatMessage>(loadedMessages, showReasoning, true) : item.messages, [loadedMessages, item.messages, showReasoning]);
  const detailCount = item.detail?.count ?? detailMessages.length;
  const detailSummary = tf('chat.detailEntries', detailCount);
  const detailAnchorId = String(item.messages[0]?.id || item.id);
  const loadDetails = () => {
    if (!item.detail || !loadTurnDetails || loading || loadedMessages.length) return;
    setLoading(true);
    setError('');
    loadTurnDetails(item.detail)
      .then(setLoadedMessages)
      .catch((reason: unknown) => setError(reason instanceof Error ? reason.message : String(reason)))
      .finally(() => setLoading(false));
  };
  return <details className="turn-detail-group" data-message-id={!open ? detailAnchorId : undefined} open={open} aria-label={detailSummary} onToggle={(event) => { const nextOpen = event.currentTarget.open; setOpen(nextOpen); if (nextOpen) loadDetails(); }}>
    <summary className="turn-detail-summary"><span className="turn-detail-copy">{detailSummary}</span><ChevronRight className="tool-chevron turn-detail-arrow" aria-hidden="true" /></summary>
    <div className="turn-detail-body">
      {loading ? t('status.loading') : null}
      {error && <p className="error-text">{error}</p>}
      {detailMessages.map((message) => <MessageView key={message.id} message={message} showReasoning={showReasoning} assistantName={assistantName} suppressMessageAnchor={!open} />)}
    </div>
  </details>;
}

function SpecialContextGroup({ item }: { item: SpecialContextGroupItem<ChatMessage> }) {
  const title = t('chat.specialContext');
  const anchorId = String(item.messages[0]?.id || item.id);
  const content = item.messages.map((message) => String(message.content || '').trim()).filter(Boolean).join('\n\n');
  return <details className="special-context-block" data-message-id={anchorId} aria-label={title}>
    <summary className="special-context-summary"><span className="special-context-copy">{title}</span><ChevronRight className="tool-chevron special-context-arrow" aria-hidden="true" /></summary>
    <pre className="special-context-body">{content}</pre>
  </details>;
}

function SessionTaskCheckbox({ status, label }: { status: SessionTaskStatus; label: string }) {
  return <input type="checkbox" className="session-task-checkbox" checked={status === 'completed'} readOnly tabIndex={-1} aria-label={label} ref={(node) => { if (node) node.indeterminate = status === 'in_progress'; }} />;
}

function SessionStateMessage({ item }: { item: SessionStateMessageItem<ChatMessage> }) {
  const state = parseSessionStateMessage(item.message.content);
  if (!state) return null;
  return <article className="session-state-message" data-message-id={item.message.id || item.id}>
    <div className="session-state-notice"><Info aria-hidden="true" /><span>{state.notice}</span></div>
    {state.details && <div className="session-state-details msg-body"><div className="md-content" dangerouslySetInnerHTML={{ __html: markdownText(state.details) }} /></div>}
    {state.tasks.length > 0 && <ul className="session-task-list">{state.tasks.map((task, index) => <li className={`session-task-item ${task.status}`} key={`${task.id}:${index}`}>
      <SessionTaskCheckbox status={task.status} label={`${task.id ? `${task.id}: ` : ''}${task.description}`} />
      <span className="session-task-copy">{task.id && <strong>{task.id}</strong>}<span>{task.description}</span></span>
    </li>)}</ul>}
  </article>;
}

function DesktopTurnBlock({ block, showReasoning, assistantName, loadTurnDetails }: { block: TurnDetailBlock<ChatMessage>; showReasoning: boolean; assistantName?: string; loadTurnDetails?: LoadTurnDetails }) {
  const sessionStateOnly = block.items.length === 1 && block.items[0]?.kind === 'sessionState';
  const historyGapOnly = block.items.length === 1 && block.items[0]?.kind === 'message' && !!block.items[0].message.historyGap;
  return <article className={`desktop-turn-block${sessionStateOnly ? ' session-state-turn-block' : ''}${historyGapOnly ? ' history-gap-turn-block' : ''}`} data-turn-block-id={block.id}>
    {block.items.map((item) => {
      if (item.kind === 'detailGroup') return <TurnDetailGroup key={item.id} item={item} showReasoning={showReasoning} assistantName={assistantName} loadTurnDetails={loadTurnDetails} />;
      if (item.kind === 'specialContextGroup') return <SpecialContextGroup key={item.id} item={item} />;
      if (item.kind === 'sessionState') return <SessionStateMessage key={item.id} item={item} />;
      return <MessageView key={item.message.id || item.sourceIndexes.join('-')} message={item.message} showReasoning={showReasoning} assistantName={assistantName} />;
    })}
  </article>;
}
