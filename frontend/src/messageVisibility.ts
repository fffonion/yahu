export type MessageVisibilityInput = {
  role: string;
  content?: string | null;
  reasoning?: string | null;
  pending?: boolean;
};

export function shouldRenderMessage(message: MessageVisibilityInput, showReasoning = false): boolean {
  if (message.role === 'tool') return true;
  if (message.pending) return true;
  if (String(message.content || '').trim()) return true;
  if (showReasoning && String(message.reasoning || '').trim()) return true;
  return false;
}
