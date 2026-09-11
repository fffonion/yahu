export type ParsedPlatformSenderMessage = {
  content: string;
  senderName?: string;
  senderId?: string;
};

const PLATFORM_SENDER_PREFIX = /^\[([^\]|\n]{1,120})\|([^\]\n]{1,80})\][ \t]*(?:\r?\n)?([\s\S]*)$/;
const NAME_ONLY_PLATFORM_SENDER_PREFIX = /^\[([^\]|\n]{1,120})\][ \t]*(?:\r?\n)?([\s\S]*)$/;
const NAME_ONLY_SENDER_SOURCES = new Set(['telegram', 'qqbot', 'weixin', 'discord', 'slack', 'whatsapp', 'signal']);
const RESERVED_NAME_ONLY_SESSION_NOTICES = [
  /^ASYNC DELEGATION BATCH COMPLETE\s*(?:--|—)\s*deleg_[A-Za-z0-9]+$/i,
  /^CONTEXT COMPACTION\s*(?:--|—)\s*REFERENCE ONLY$/i,
  /^OUT-OF-BAND USER MESSAGE\b/i,
  /^IMPORTANT:/i,
  /^System note:/i,
  /^Session state restored$/i,
  /^Your active task list was preserved across context compression$/i,
  /^Continuing toward your standing goal$/i,
];
const RESERVED_SESSION_NOTICE_CONTENT = /^\s*\[?(?:ASYNC DELEGATION BATCH COMPLETE\s*(?:--|—)\s*deleg_[A-Za-z0-9]+|CONTEXT COMPACTION\s*(?:--|—)\s*REFERENCE ONLY|OUT-OF-BAND USER MESSAGE\b|IMPORTANT:|System note:|Session state restored\b|Your active task list was preserved across context compression\b|Continuing toward your standing goal\b)/i;

function isReservedNameOnlySessionNotice(value: string): boolean {
  return RESERVED_NAME_ONLY_SESSION_NOTICES.some((pattern) => pattern.test(value));
}

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
  const nameOnlyMatch = text.match(NAME_ONLY_PLATFORM_SENDER_PREFIX);
  if (!nameOnlyMatch) return { content: text };
  const senderName = nameOnlyMatch[1].trim();
  if (!senderName || isReservedNameOnlySessionNotice(senderName)) return { content: text };
  if (!allowNameOnly && !RESERVED_SESSION_NOTICE_CONTENT.test(nameOnlyMatch[2])) return { content: text };
  return { senderName, content: nameOnlyMatch[2] };
}
