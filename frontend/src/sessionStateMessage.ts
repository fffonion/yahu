export type SessionTaskStatus = 'pending' | 'in_progress' | 'completed' | 'cancelled';

export type SessionTaskItem = {
  id: string;
  description: string;
  status: SessionTaskStatus;
};

export type SessionStateContent = {
  notice: string;
  tasks: SessionTaskItem[];
  details?: string;
  collapsible?: boolean;
};

const taskLine = /^-\s+\[([ xX>~-])\]\s+(.+?)(?:\s+\((pending|in_progress|completed|cancelled)\))?\s*$/;
const asyncDelegationCompleteNotice = /^ASYNC DELEGATION BATCH COMPLETE\s*(?:--|—)\s*deleg_[A-Za-z0-9]+$/i;
const contextCompactionNotice = /^CONTEXT COMPACTION\s*(?:--|—)\s*REFERENCE ONLY$/i;
const backgroundProcessNotice = /^\[IMPORTANT:\s*Background process\s+(\S+)\s+(.+?)(?:\s+\(exit code\s+[^)]*\))?\.\]?[ \t]*(?:\r?\n|$)([\s\S]*)$/i;

function statusFromMarker(marker: string): SessionTaskStatus {
  if (marker === '>') return 'in_progress';
  if (marker.toLowerCase() === 'x') return 'completed';
  if (marker === '-' || marker === '~') return 'cancelled';
  return 'pending';
}

function parseBackgroundProcessNotice(content: string): SessionStateContent | null {
  const match = content.match(backgroundProcessNotice);
  if (!match) return null;
  const processId = match[1].trim();
  const status = match[2].trim();
  const details = String(match[3] || '').replace(/\s*\]\s*$/, '').trim();
  return {
    notice: `Background process ${processId} ${status}`,
    tasks: [],
    collapsible: true,
    ...(details ? { details } : {}),
  };
}

export function isBackgroundProcessNotice(content: string): boolean {
  return backgroundProcessNotice.test(content);
}

export function parseSessionStateMessage(content: string): SessionStateContent | null {
  const normalizedContent = String(content || '').replace(/\r\n?/g, '\n');
  const backgroundProcess = parseBackgroundProcessNotice(normalizedContent);
  if (backgroundProcess) return backgroundProcess;
  const lines = normalizedContent.split('\n');
  const firstLine = lines[0] || '';
  const details = lines.slice(1).join('\n').trim();
  if (asyncDelegationCompleteNotice.test(firstLine)) return { notice: firstLine, tasks: [], collapsible: true, ...(details ? { details } : {}) };
  const noticeMatch = firstLine.match(/^\[([^\]\r\n]+)\]$/);
  if (!noticeMatch || noticeMatch[1].includes('|')) return null;
  const notice = noticeMatch[1].trim();
  if (asyncDelegationCompleteNotice.test(notice)) return { notice, tasks: [], collapsible: true, ...(details ? { details } : {}) };
  if (contextCompactionNotice.test(notice)) return { notice, tasks: [], collapsible: true, ...(details ? { details } : {}) };

  const tasks: SessionTaskItem[] = [];
  for (const line of lines.slice(1)) {
    if (!line.trim()) continue;
    const match = line.match(taskLine);
    if (!match) return { notice, tasks: [], ...(details ? { details } : {}) };
    const body = match[2].trim();
    const idMatch = body.match(/^([A-Za-z0-9][\w-]*)\.\s+(.+)$/);
    tasks.push({
      id: idMatch?.[1] || '',
      description: idMatch?.[2] || body,
      status: (match[3] as SessionTaskStatus | undefined) || statusFromMarker(match[1]),
    });
  }

  return { notice, tasks };
}

export function isSessionStateMessage(message: { role?: string | null; content?: string | null }): boolean {
  return (message.role === 'user' || message.role === 'system') && parseSessionStateMessage(String(message.content || '')) !== null;
}
