export type MessageVisibilityInput = {
  id?: string;
  role: string;
  content?: string | null;
  reasoning?: string | null;
  pending?: boolean;
  toolName?: string | null;
  toolInput?: unknown;
  toolCalls?: unknown;
  toolCallId?: string | null;
};

function recordLooksToolLike(value: unknown): boolean {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  return ['tool_name', 'tool', 'recipient_name', 'tool_call', 'tool_input', 'tool_args'].some((key) => record[key] !== undefined);
}

function contentLooksToolLike(content?: string | null): boolean {
  const text = String(content || '').trim();
  if (!text) return false;
  if (text.includes('<untrusted_tool_result')) return true;
  if (!text.startsWith('{') && !text.startsWith('[')) return false;
  try {
    const parsed = JSON.parse(text);
    if (recordLooksToolLike(parsed)) return true;
    if (Array.isArray(parsed)) return parsed.some(recordLooksToolLike);
  } catch { /* not JSON */ }
  return false;
}

function hasVisibleContent(message: MessageVisibilityInput): boolean {
  return !!String(message.content || '').trim();
}

function hasToolCalls(value: unknown): boolean {
  if (Array.isArray(value)) return value.length > 0;
  return value !== undefined && value !== null;
}

export function isAssistantToolPreludeMessage(message: MessageVisibilityInput): boolean {
  return message.role === 'assistant' && hasVisibleContent(message) && hasToolCalls(message.toolCalls);
}

export function isToolLikeMessage(message: MessageVisibilityInput): boolean {
  if (message.role === 'tool') return true;
  if (isAssistantToolPreludeMessage(message)) return false;
  if (String(message.toolName || '').trim()) return true;
  if (message.toolInput !== undefined && message.toolInput !== null) return true;
  if (Array.isArray(message.toolCalls) && message.toolCalls.length > 0) return true;
  if (message.toolCalls !== undefined && message.toolCalls !== null && !Array.isArray(message.toolCalls)) return true;
  if (message.role !== 'user' && contentLooksToolLike(message.content)) return true;
  return false;
}

function isEmptyAssistantToolCallPlaceholder(message: MessageVisibilityInput): boolean {
  return message.role === 'assistant'
    && !message.pending
    && !String(message.content || '').trim()
    && hasToolCalls(message.toolCalls);
}

export function shouldRenderMessage(message: MessageVisibilityInput, showReasoning = false, showToolCalls = true): boolean {
  if (isEmptyAssistantToolCallPlaceholder(message)) return false;
  if (isToolLikeMessage(message)) return showToolCalls;
  if (message.pending) return true;
  if (String(message.content || '').trim()) return true;
  if (showReasoning && String(message.reasoning || '').trim()) return true;
  return false;
}

function isLocalStreamAssistantMessage(message: MessageVisibilityInput): boolean {
  return message.role === 'assistant' && String(message.id || '').startsWith('assistant_');
}

function parseMaybeJsonValue(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  const trimmed = value.trim();
  if (!trimmed || !/^[\[{]/.test(trimmed)) return value;
  try { return JSON.parse(trimmed); } catch { return value; }
}

function recordValue(record: Record<string, unknown> | null, keys: string[]): unknown {
  if (!record) return undefined;
  for (const key of keys) {
    const value = record[key];
    if (value !== undefined && value !== null && value !== '') return parseMaybeJsonValue(value);
  }
  return undefined;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function rememberToolCallInputs(message: MessageVisibilityInput, inputs: Map<string, unknown>) {
  if (message.role !== 'assistant' || !Array.isArray(message.toolCalls)) return;
  for (const raw of message.toolCalls) {
    const call = asRecord(raw);
    const fn = asRecord(call?.function);
    const input = recordValue(call, ['arguments', 'args', 'params', 'parameters', 'input', 'tool_input', 'tool_args', 'request'])
      ?? recordValue(fn, ['arguments', 'args', 'params', 'parameters']);
    if (input === undefined) continue;
    for (const id of [call?.call_id, call?.id, call?.tool_call_id]) {
      const key = String(id || '').trim();
      if (key) inputs.set(key, input);
    }
  }
}

export function dedupeVisibleChatMessages<T extends MessageVisibilityInput>(messages: T[]): T[] {
  const result: T[] = [];
  const assistantByTurnContent = new Map<string, number>();
  const toolInputsByCallId = new Map<string, unknown>();
  let turn = 0;
  let lastUserResultIndex = -1;
  for (const rawMessage of messages) {
    let msg = rawMessage;
    rememberToolCallInputs(msg, toolInputsByCallId);
    const toolCallId = String(msg.toolCallId || '').trim();
    if (msg.role === 'tool' && msg.toolInput === undefined && toolCallId && toolInputsByCallId.has(toolCallId)) {
      msg = { ...msg, toolInput: toolInputsByCallId.get(toolCallId) };
    }
    if (msg.role === 'user') {
      turn += 1;
      assistantByTurnContent.clear();
      lastUserResultIndex = result.length;
      result.push(msg);
      continue;
    }
    const isAssistantAnswer = msg.role === 'assistant' && String(msg.content || '').trim() && !isToolLikeMessage(msg);
    if (isAssistantAnswer) {
      const fuzzyExisting = result.findIndex((m, i) => {
        if (i <= lastUserResultIndex || m.role !== 'assistant' || isToolLikeMessage(m)) return false;
        return !result.slice(i + 1).some((later) => isToolLikeMessage(later));
      });
      if (fuzzyExisting >= 0) {
        const previous = result[fuzzyExisting];
        const preferCurrent = (isLocalStreamAssistantMessage(previous) && !isLocalStreamAssistantMessage(msg)) || String(msg.content || '').length > String(previous.content || '').length;
        result[fuzzyExisting] = preferCurrent ? { ...previous, ...msg, pending: Boolean(previous.pending && msg.pending) } : { ...previous, pending: Boolean(previous.pending && msg.pending) };
        continue;
      }
      const key = `${turn}\u0000${String(msg.content || '').trim()}`;
      const existing = assistantByTurnContent.get(key);
      if (existing !== undefined) {
        const previous = result[existing];
        const preferCurrent = isLocalStreamAssistantMessage(previous) && !isLocalStreamAssistantMessage(msg);
        result[existing] = preferCurrent ? { ...previous, ...msg, pending: Boolean(previous.pending && msg.pending) } : { ...previous, pending: Boolean(previous.pending && msg.pending) };
        continue;
      }
      assistantByTurnContent.set(key, result.length);
    }
    result.push(msg);
  }
  return result;
}

export function renderableMessages<T extends MessageVisibilityInput>(messages: T[], showReasoning = false, showToolCalls = true): T[] {
  return messages.filter((message) => shouldRenderMessage(message, showReasoning, showToolCalls));
}
