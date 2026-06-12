export type ParsedPlatformSenderMessage = {
  content: string;
  senderName?: string;
  senderId?: string;
};

const PLATFORM_SENDER_PREFIX = /^\[([^\]|\n]{1,120})\|([^\]\n]{1,80})\][ \t]*(?:\r?\n)?([\s\S]*)$/;

export function parsePlatformSenderMessage(content: string): ParsedPlatformSenderMessage {
  const text = String(content || '');
  const match = text.match(PLATFORM_SENDER_PREFIX);
  if (!match) return { content: text };
  const senderName = match[1].trim();
  const senderId = match[2].trim();
  if (!senderName || !senderId) return { content: text };
  return { senderName, senderId, content: match[3] };
}
