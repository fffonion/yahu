export type SessionPreviewMessage = { role?: string; content?: string | null; pending?: boolean };

export function compactSessionPreview(text: string) {
  return text.replace(/\s+/g, ' ').trim();
}

export function latestSessionPreviewFromMessages(messages: SessionPreviewMessage[]) {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    const content = compactSessionPreview(String(message?.content || ''));
    if (!content) continue;
    if (message.role === 'assistant' || message.role === 'user') return content;
  }
  return '';
}
