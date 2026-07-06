export type SessionTimeLike = {
  title?: string | null;
  started_at?: number | string | null;
  last_active?: number | string | null;
  ended_at?: number | string | null;
};

export type MessageTimeLike = {
  timestamp?: number | string | null;
};

export type TimeFormatter = (date: Date) => string;

function timestampMs(value: unknown): number | null {
  if (value === null || value === undefined || value === '') return null;
  if (typeof value === 'string') {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const numeric = Number(trimmed);
    if (Number.isFinite(numeric) && numeric > 0) return numeric < 1e12 ? numeric * 1000 : numeric;
    const parsed = Date.parse(trimmed);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
  }
  const numeric = Number(value);
  if (!Number.isFinite(numeric) || numeric <= 0) return null;
  return numeric < 1e12 ? numeric * 1000 : numeric;
}

export function defaultSessionTimeFormatter(date: Date) {
  return date.toLocaleString([], {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatSessionTime(value: unknown, formatter: TimeFormatter = defaultSessionTimeFormatter) {
  const ms = timestampMs(value);
  return ms === null ? '' : formatter(new Date(ms));
}

function pad2(value: number) {
  return String(value).padStart(2, '0');
}

function localDateKey(date: Date) {
  return `${date.getFullYear()}-${pad2(date.getMonth() + 1)}-${pad2(date.getDate())}`;
}

export function formatChatMessageTime(value: unknown, now: Date = new Date()) {
  const ms = timestampMs(value) ?? now.getTime();
  const date = new Date(ms);
  const time = `${pad2(date.getHours())}:${pad2(date.getMinutes())}`;
  if (localDateKey(date) === localDateKey(now)) return time;
  return `${pad2(date.getMonth() + 1)}/${pad2(date.getDate())} ${time}`;
}

export function sessionDisplayTitle(session: SessionTimeLike | null | undefined, formatter: TimeFormatter = defaultSessionTimeFormatter) {
  const title = String(session?.title || '').trim();
  if (title) return title;
  return formatSessionTime(session?.started_at, formatter) || '—';
}

export function sessionHeaderTimes(session: SessionTimeLike | null | undefined, messages: MessageTimeLike[] = [], formatter: TimeFormatter = defaultSessionTimeFormatter) {
  const startedMs = timestampMs(session?.started_at);
  const latestCandidates = [
    timestampMs(session?.last_active),
    timestampMs(session?.ended_at),
    startedMs,
    ...messages.map((message) => timestampMs(message.timestamp)),
  ].filter((ms): ms is number => ms !== null);
  const latestMs = latestCandidates.length ? Math.max(...latestCandidates) : null;
  return {
    started: startedMs !== null ? `Started ${formatter(new Date(startedMs))}` : '',
    latest: latestMs !== null ? `Latest ${formatter(new Date(latestMs))}` : '',
  };
}
