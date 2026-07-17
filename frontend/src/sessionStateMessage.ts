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
};

const taskLine = /^-\s+\[([ xX>~-])\]\s+(.+?)(?:\s+\((pending|in_progress|completed|cancelled)\))?\s*$/;
const asyncDelegationCompleteNotice = /^ASYNC DELEGATION BATCH COMPLETE\s*(?:--|—)\s*deleg_[A-Za-z0-9]+$/i;

function statusFromMarker(marker: string): SessionTaskStatus {
  if (marker === '>') return 'in_progress';
  if (marker.toLowerCase() === 'x') return 'completed';
  if (marker === '-' || marker === '~') return 'cancelled';
  return 'pending';
}

export function parseSessionStateMessage(content: string): SessionStateContent | null {
  const lines = String(content || '').replace(/\r\n?/g, '\n').split('\n');
  const noticeMatch = lines[0]?.match(/^\[([^\]\r\n]+)\]$/);
  if (!noticeMatch || noticeMatch[1].includes('|')) return null;
  const notice = noticeMatch[1].trim();
  const details = lines.slice(1).join('\n').trim();
  if (asyncDelegationCompleteNotice.test(notice)) return { notice, tasks: [], ...(details ? { details } : {}) };

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
  return message.role === 'user' && parseSessionStateMessage(String(message.content || '')) !== null;
}
