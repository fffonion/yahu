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

export type SubagentMessage = {
  id: string;
  role: 'user' | 'assistant' | 'tool' | 'system';
  content: string;
  reasoning?: string;
  toolName?: string;
  toolCalls?: string;
  timestamp?: number;
};

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
    items.sort((a, b) => (a.startedAt || 0) - (b.startedAt || 0));
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
  const raw = value as Record<string, unknown>;
  const rawRole = stringValue(raw.role);
  const role = rawRole === 'user' || rawRole === 'tool' || rawRole === 'system' ? rawRole : 'assistant';
  const content = textValue(raw.content);
  const reasoning = textValue(raw.reasoning) || textValue(raw.reasoning_content);
  const toolName = stringValue(raw.tool_name) || undefined;
  const toolCalls = Array.isArray(raw.tool_calls) && raw.tool_calls.length
    ? JSON.stringify(raw.tool_calls, null, 2)
    : undefined;
  if (!content && !reasoning && !toolName && !toolCalls) return null;
  return {
    id: raw.id == null ? String(index) : String(raw.id),
    role,
    content,
    reasoning: reasoning || undefined,
    toolName,
    toolCalls,
    timestamp: numberValue(raw.timestamp),
  };
}

function normalizeStatus(value: unknown): SubagentStatus {
  const status = stringValue(value);
  return status === 'completed' || status === 'failed' || status === 'interrupted' || status === 'timeout' ? status : 'running';
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

function textValue(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function numberValue(value: unknown): number | undefined {
  const number = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : NaN;
  return Number.isFinite(number) ? number : undefined;
}

function integerValue(value: unknown): number {
  return Math.max(0, Math.trunc(numberValue(value) || 0));
}
