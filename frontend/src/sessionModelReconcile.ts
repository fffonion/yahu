type SessionModelFields = {
  id: string;
  model?: string;
  provider?: string;
  ended_at?: number | string | null;
  last_active?: number | string;
};

export function preferNewerSessionModel<T extends SessionModelFields>(next: T, current?: SessionModelFields | null): T {
  const ended = Number(next.ended_at);
  const active = Number(current?.last_active);
  if (!current || current.id !== next.id || next.ended_at == null || !Number.isFinite(ended)
    || !Number.isFinite(active) || active <= ended || !current.model?.trim()) return next;
  return { ...next, model: current.model, provider: current.provider, ended_at: current.ended_at ?? null };
}
