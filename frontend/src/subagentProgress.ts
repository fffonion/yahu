import type { ChatMessage } from './ChatTranscript';
import { normalizeChatMessage } from './chatMessage';

export type SubagentStatus = 'running' | 'completed' | 'failed' | 'interrupted' | 'timeout';
export type SubagentTodoStatus = 'pending' | 'in_progress' | 'completed' | 'cancelled';

export type SubagentTodo = {
  id: string;
  content: string;
  status: SubagentTodoStatus;
};

export type SubagentActivity = {
  tool: string;
  timestamp?: number;
};

export type SubagentMessage = ChatMessage;

export type SubagentProgress = {
  sessionId: string;
  parentSessionId: string;
  goal: string;
  model?: string;
  status: SubagentStatus;
  startedAt?: number;
  endedAt?: number;
  messageCount: number;
  toolCount: number;
  apiCalls: number;
  currentTool?: string;
  todos: SubagentTodo[];
  activity: SubagentActivity[];
  summary?: string;
};

export type SubagentProgressSnapshot = {
  sessionId: string;
  generatedAt?: number;
  subagents: SubagentProgress[];
  error?: string;
};

export type SubagentTreeNode = SubagentProgress & { children: SubagentTreeNode[] };

type WebSocketLocation = Pick<Location, 'protocol' | 'host'>;

export function subagentWebSocketUrl(location: WebSocketLocation, sessionId: string): string {
  const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${scheme}//${location.host}/chat/subagents/${encodeURIComponent(sessionId)}/ws`;
}

export function subagentMessagesUrl(sessionId: string): string {
  return `/chat/subagents/${encodeURIComponent(sessionId)}/messages`;
}

export function normalizeSubagentMessages(value: unknown): SubagentMessage[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  const data = (value as Record<string, unknown>).data;
  if (!Array.isArray(data)) return [];
  return data.map((item, index) => normalizeSubagentMessage(item, index)).filter((item): item is SubagentMessage => !!item);
}

export function mergeSubagentMessages(previous: SubagentMessage[], incoming: SubagentMessage[]): SubagentMessage[] {
  if (!previous.length) return incoming;
  const previousById = new Map(previous.map((message) => [message.id, message]));
  let changed = previous.length !== incoming.length;
  const merged = incoming.map((message, index) => {
    const current = previousById.get(message.id);
    if (!current || !sameSubagentMessage(current, message)) {
      changed = true;
      return message;
    }
    if (previous[index] !== current) changed = true;
    return current;
  });
  return changed ? merged : previous;
}

function sameSubagentMessage(previous: SubagentMessage, incoming: SubagentMessage): boolean {
  return previous.id === incoming.id
    && previous.role === incoming.role
    && previous.content === incoming.content
    && previous.reasoning === incoming.reasoning
    && previous.timestamp === incoming.timestamp
    && previous.pending === incoming.pending
    && previous.toolName === incoming.toolName
    && previous.toolCallId === incoming.toolCallId
    && previous.tokenCount === incoming.tokenCount
    && previous.model === incoming.model
    && previous.provider === incoming.provider
    && previous.platformSenderName === incoming.platformSenderName
    && previous.platformSenderId === incoming.platformSenderId
    && sameSubagentMessageField(previous.structuredContent, incoming.structuredContent)
    && sameSubagentMessageField(previous.toolInput, incoming.toolInput)
    && sameSubagentMessageField(previous.toolCalls, incoming.toolCalls)
    && sameSubagentMessageField(previous.turnMetrics, incoming.turnMetrics)
    && sameSubagentMessageField(previous.turnDetails, incoming.turnDetails);
}

function sameSubagentMessageField(previous: unknown, incoming: unknown): boolean {
  return previous === incoming || JSON.stringify(previous) === JSON.stringify(incoming);
}

export function parseSubagentFinalStructuredContent(content: string): SubagentMessage['structuredContent'] | undefined {
  const trimmed = content.trim();
  if (!trimmed) return undefined;
  try {
    return { value: JSON.parse(trimmed) };
  } catch {
    return undefined;
  }
}

export function formatSubagentFinalMessages(messages: SubagentMessage[]): SubagentMessage[] {
  let finalAssistantIndex = -1;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index];
    if (message.role === 'assistant' && message.content.trim()) {
      finalAssistantIndex = index;
      break;
    }
  }
  if (finalAssistantIndex < 0) return messages;
  const message = messages[finalAssistantIndex];
  const structuredContent = parseSubagentFinalStructuredContent(message.content);
  if (!structuredContent) return messages;
  return messages.map((item, index) => index === finalAssistantIndex ? { ...item, structuredContent } : item);
}

export function normalizeSubagentSnapshot(value: unknown, expectedSessionId: string): SubagentProgressSnapshot | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const raw = value as Record<string, unknown>;
  if (raw.type !== 'subagents.snapshot' || stringValue(raw.session_id) !== expectedSessionId || !Array.isArray(raw.subagents)) return null;

  const subagents = raw.subagents.map(normalizeSubagent).filter((item): item is SubagentProgress => !!item);
  return {
    sessionId: expectedSessionId,
    generatedAt: numberValue(raw.generated_at),
    subagents,
    error: stringValue(raw.error) || undefined,
  };
}

export function latestSubagent(subagents: SubagentProgress[]): SubagentProgress | undefined {
  return subagents.reduce<SubagentProgress | undefined>((latest, item) => !latest || (item.startedAt || 0) >= (latest.startedAt || 0) ? item : latest, undefined);
}

export function previewSubagent(subagents: SubagentProgress[]): SubagentProgress | undefined {
  const running = subagents.filter((item) => item.status === 'running');
  return latestSubagent(running.length > 0 ? running : subagents);
}

export function isSubagentDetailNearBottom(metrics: Pick<HTMLElement, 'scrollTop' | 'scrollHeight' | 'clientHeight'>, thresholdPx = 96): boolean {
  return metrics.scrollHeight - metrics.scrollTop - metrics.clientHeight <= thresholdPx;
}

export function buildSubagentTree(subagents: SubagentProgress[], parentSessionId: string): SubagentTreeNode[] {
  const nodes = new Map<string, SubagentTreeNode>();
  for (const subagent of subagents) nodes.set(subagent.sessionId, { ...subagent, children: [] });

  const roots: SubagentTreeNode[] = [];
  for (const node of nodes.values()) {
    const parent = nodes.get(node.parentSessionId);
    if (parent && parent !== node) parent.children.push(node);
    else if (node.parentSessionId === parentSessionId || !parent) roots.push(node);
  }
  const sortTree = (items: SubagentTreeNode[]) => {
    items.sort((a, b) => (b.startedAt || 0) - (a.startedAt || 0));
    items.forEach((item) => sortTree(item.children));
  };
  sortTree(roots);
  return roots;
}

export function subagentElapsedSeconds(subagent: SubagentProgress, nowSeconds: number): number {
  if (!subagent.startedAt) return 0;
  return Math.max(0, Math.round((subagent.endedAt || nowSeconds) - subagent.startedAt));
}

export function formatSubagentElapsed(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = whole % 60;
  if (hours > 0) return `${hours}h ${minutes}m ${rest}s`;
  if (minutes > 0) return `${minutes}m ${rest}s`;
  return `${rest}s`;
}

function normalizeSubagent(value: unknown): SubagentProgress | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const raw = value as Record<string, unknown>;
  const sessionId = stringValue(raw.session_id);
  if (!sessionId) return null;
  return {
    sessionId,
    parentSessionId: stringValue(raw.parent_session_id),
    goal: stringValue(raw.goal) || 'Subagent',
    model: stringValue(raw.model) || undefined,
    status: normalizeStatus(raw.status),
    startedAt: numberValue(raw.started_at),
    endedAt: numberValue(raw.ended_at),
    messageCount: integerValue(raw.message_count),
    toolCount: integerValue(raw.tool_count),
    apiCalls: integerValue(raw.api_calls),
    currentTool: stringValue(raw.current_tool) || undefined,
    todos: Array.isArray(raw.todos) ? raw.todos.map(normalizeTodo).filter((item): item is SubagentTodo => !!item) : [],
    activity: Array.isArray(raw.activity) ? raw.activity.map(normalizeActivity).filter((item): item is SubagentActivity => !!item) : [],
    summary: stringValue(raw.summary) || undefined,
  };
}

function normalizeTodo(value: unknown): SubagentTodo | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const raw = value as Record<string, unknown>;
  const content = stringValue(raw.content);
  if (!content) return null;
  const status = stringValue(raw.status);
  return {
    id: stringValue(raw.id),
    content,
    status: status === 'completed' || status === 'in_progress' || status === 'cancelled' ? status : 'pending',
  };
}

function normalizeActivity(value: unknown): SubagentActivity | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const raw = value as Record<string, unknown>;
  const tool = stringValue(raw.tool);
  if (!tool) return null;
  return { tool, timestamp: numberValue(raw.timestamp) };
}

function normalizeSubagentMessage(value: unknown, index: number): SubagentMessage | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return normalizeChatMessage(value, String(index));
}

function normalizeStatus(value: unknown): SubagentStatus {
  const status = stringValue(value);
  return status === 'completed' || status === 'failed' || status === 'interrupted' || status === 'timeout' ? status : 'running';
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function numberValue(value: unknown): number | undefined {
  const number = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : NaN;
  return Number.isFinite(number) ? number : undefined;
}

function integerValue(value: unknown): number {
  return Math.max(0, Math.trunc(numberValue(value) || 0));
}
