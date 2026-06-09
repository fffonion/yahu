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

export function sessionDisplayTitle(session: SessionTimeLike | null | undefined, formatter: TimeFormatter = defaultSessionTimeFormatter) {
  const title = String(session?.title || '').trim();
  if (title) return title;
  return formatSessionTime(session?.started_at, formatter) || '—';
}

export function sessionHeaderTimes(session: SessionTimeLike | null | undefined, messages: MessageTimeLike[] = [], formatter: TimeFormatter = defaultSessionTimeFormatter) {
  const started = formatSessionTime(session?.started_at, formatter);
  const latestMessageMs = messages.reduce<number | null>((latest, message) => {
    const ms = timestampMs(message.timestamp);
    if (ms === null) return latest;
    return latest === null || ms > latest ? ms : latest;
  }, null);
  const latest = latestMessageMs !== null
    ? formatter(new Date(latestMessageMs))
    : formatSessionTime(session?.last_active ?? session?.ended_at ?? session?.started_at, formatter);
  return {
    started: started ? `Started ${started}` : '',
    latest: latest ? `Latest ${latest}` : '',
  };
}
