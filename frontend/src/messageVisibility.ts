export type MessageVisibilityInput = {
  role: string;
  content?: string | null;
  reasoning?: string | null;
  pending?: boolean;
  toolName?: string | null;
  toolInput?: unknown;
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

export function isToolLikeMessage(message: MessageVisibilityInput): boolean {
  if (message.role === 'tool') return true;
  if (String(message.toolName || '').trim()) return true;
  if (message.toolInput !== undefined && message.toolInput !== null) return true;
  if (message.role !== 'user' && contentLooksToolLike(message.content)) return true;
  return false;
}

export function shouldRenderMessage(message: MessageVisibilityInput, showReasoning = false, showToolCalls = true): boolean {
  if (isToolLikeMessage(message)) return showToolCalls;
  if (message.pending) return true;
  if (String(message.content || '').trim()) return true;
  if (showReasoning && String(message.reasoning || '').trim()) return true;
  return false;
}
