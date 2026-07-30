export type ParsedPlatformSenderMessage = {
  content: string;
  senderName?: string;
  senderId?: string;
};

const PLATFORM_SENDER_PREFIX = /^\[([^\]|\n]{1,120})\|([^\]\n]{1,80})\][ \t]*(?:\r?\n)?([\s\S]*)$/;
const NAME_ONLY_PLATFORM_SENDER_PREFIX = /^\[([^\]|\n]{1,120})\][ \t]*(?:\r?\n)?([\s\S]*)$/;
const NAME_ONLY_SENDER_SOURCES = new Set(['telegram', 'qqbot', 'weixin', 'discord', 'slack', 'whatsapp', 'signal']);

export function platformSourceUsesNameOnlySenderPrefix(source: unknown): boolean {
  return NAME_ONLY_SENDER_SOURCES.has(String(source || '').trim().toLowerCase());
}

export function parsePlatformSenderMessage(content: string, allowNameOnly = false): ParsedPlatformSenderMessage {
  const text = String(content || '');
  const match = text.match(PLATFORM_SENDER_PREFIX);
  if (match) {
    const senderName = match[1].trim();
    const senderId = match[2].trim();
    if (senderName && senderId) return { senderName, senderId, content: match[3] };
  }
  if (!allowNameOnly) return { content: text };
  const nameOnlyMatch = text.match(NAME_ONLY_PLATFORM_SENDER_PREFIX);
  if (!nameOnlyMatch) return { content: text };
  const senderName = nameOnlyMatch[1].trim();
  if (!senderName) return { content: text };
  return { senderName, content: nameOnlyMatch[2] };
}
