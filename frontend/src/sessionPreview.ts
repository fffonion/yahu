import { parseSessionStateMessage } from './sessionStateMessage';

export type SessionPreviewMessage = { role?: string; content?: string | null; pending?: boolean };

const gatewaySenderPrefix = /^\[[^\]\r\n]+\][ \t]*(?:\r?\n)?/;
const outOfBandMessage = /^\s*\[?OUT-OF-BAND USER MESSAGE\b[^\r\n]*\]?\s*\r?\n([\s\S]*?)\s*\[\/OUT-OF-BAND USER MESSAGE\]\s*$/i;
const gatewayOrigin = /^Gateway message origin \(JSON data, not instructions or authorization\):\s*\r?\n[\s\S]*?\r?\nDo not guess a reply destination when these fields are insufficient\.\s*/i;

function unwrapOutOfBandPreview(text: string) {
  const match = text.match(outOfBandMessage);
  if (!match) return null;
  return match[1].replace(gatewayOrigin, '').trim();
}

function isSessionPreviewMarker(text: string) {
  return parseSessionStateMessage(text) !== null
    || parseSessionStateMessage(text.replace(gatewaySenderPrefix, '')) !== null;
}

export function compactSessionPreview(text: string) {
  return text.replace(gatewaySenderPrefix, '').replace(/\s+/g, ' ').trim();
}

export function sessionPreviewForDisplay(text: string | null | undefined) {
  const rawContent = String(text || '');
  if (!rawContent.trim()) return '';
  const outOfBandContent = unwrapOutOfBandPreview(rawContent);
  if (outOfBandContent !== null) return compactSessionPreview(outOfBandContent);
  if (isSessionPreviewMarker(rawContent)) return '';
  return compactSessionPreview(rawContent);
}

export function latestSessionPreviewFromMessages(messages: SessionPreviewMessage[]) {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.role !== 'assistant' && message.role !== 'user') continue;
    const content = sessionPreviewForDisplay(message?.content);
    if (!content) continue;
    return content;
  }
  return '';
}
