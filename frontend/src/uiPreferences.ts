import { useEffect, useState } from 'react';

const legacyKeys = {
  pinned_sessions: 'pinnedSessions',
  enabled_providers: 'yahu.provider-usage.enabled.v1',
  provider_order: 'yahu.provider-usage.order.v1',
} as const;
type Field = keyof typeof legacyKeys;
const fields = Object.keys(legacyKeys) as Field[];
export type PreferenceValues = Record<Field, string[]>;
type Snapshot = { revision: number } & Record<Field, string[] | null>;
type Patch = { revision: number } & Partial<PreferenceValues>;
type PreferenceRequest = (patch?: Patch) => Promise<{ preferences: Snapshot; conflict: boolean }>;
type Change = { field: Field; apply: (value: string[]) => string[] };

export function readLegacyPreferences(read: (key: string) => string | null) {
  const result: Partial<PreferenceValues> = {};
  for (const field of fields) {
    try {
      const raw = read(legacyKeys[field]);
      if (raw === null) continue;
      const value: unknown = JSON.parse(raw);
      const items = field === 'enabled_providers'
        ? value && typeof value === 'object' && !Array.isArray(value)
          ? Object.entries(value).filter(([, enabled]) => enabled === true).map(([id]) => id)
          : []
        : Array.isArray(value) ? value : [];
      const ids = [...new Set(items.filter((id): id is string => typeof id === 'string' && id.length > 0 && id.length <= 256 && id.trim() === id && !/[,\x00-\x1f\x7f]/.test(id)))];
      // A browser with no meaningful legacy setting must not claim an unset field.
      if (ids.length) result[field] = ids.slice(0, field === 'pinned_sessions' ? 100 : 256);
    } catch { /* missing or inaccessible legacy storage */ }
  }
  return result;
}

const sameIds = (a: string[], b: string[]) => a.length === b.length && a.every((id, i) => id === b[i]);

export function createPreferenceSync(legacy: Partial<PreferenceValues>, publish: (value: PreferenceValues) => void, request: PreferenceRequest) {
  let remote: Snapshot | undefined;
  let pending: Change[] = [];
  let active: Promise<void> | undefined;
  let refreshRequested = false;
  let view: PreferenceValues = { pinned_sessions: [], enabled_providers: [], provider_order: [], ...legacy };

  const render = () => {
    const next = { ...view };
    for (const field of fields) {
      const base = remote?.[field] ?? legacy[field] ?? [];
      const ids = pending.filter(change => change.field === field).reduce((ids, change) => change.apply(ids), base);
      if (!sameIds(view[field], ids)) next[field] = ids;
    }
    if (fields.some(field => view[field] !== next[field])) {
      view = next;
      publish(view);
    }
  };

  const drain = async () => {
    if (!remote || refreshRequested) {
      refreshRequested = false;
      remote = (await request()).preferences;
      render();
    }
    let conflicts = 0;
    while (conflicts < 4) {
      const batch = pending;
      const patch: Patch = { revision: remote.revision };
      for (const field of fields) {
        const saved = remote[field];
        const base = saved ?? legacy[field] ?? [];
        const changes = batch.filter(change => change.field === field);
        const next = changes.reduce((ids, change) => change.apply(ids), base);
        if ((saved === null && (legacy[field] !== undefined || changes.length > 0)) || !sameIds(base, next)) patch[field] = next;
      }
      if (!fields.some(field => patch[field] !== undefined)) {
        pending = pending.slice(batch.length);
        render();
        return;
      }
      const response = await request(patch);
      remote = response.preferences;
      if (response.conflict) conflicts += 1;
      else {
        conflicts = 0;
        pending = pending.slice(batch.length);
      }
      render();
    }
  };

  const flush = () => {
    if (!active) active = drain().finally(() => { active = undefined; });
    return active;
  };
  return {
    current: () => view,
    refresh: () => {
      refreshRequested = true;
      return flush();
    },
    change: (field: Field, apply: Change['apply']) => {
      if (sameIds(view[field], apply(view[field]))) return Promise.resolve();
      pending = [...pending, { field, apply }];
      render();
      return flush();
    },
  };
}

const requestPreferences: PreferenceRequest = async patch => {
  const abort = new AbortController();
  const timer = setTimeout(() => abort.abort(), 8000);
  const response = await fetch('/ui-preferences', {
    cache: 'no-store',
    signal: abort.signal,
    ...(patch && { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(patch) }),
  }).finally(() => clearTimeout(timer));
  if (!response.ok && response.status !== 409) throw new Error(`Yahu preferences: ${response.status}`);
  const preferences: Snapshot = await response.json();
  if (!Number.isSafeInteger(preferences?.revision) || fields.some(field => preferences[field] !== null && !Array.isArray(preferences[field]))) {
    throw new Error('Invalid Yahu preferences');
  }
  return { preferences, conflict: response.status === 409 };
};

export function useUiPreferences() {
  const [view, setView] = useState<PreferenceValues | null>(null);
  const [sync] = useState(() => createPreferenceSync(readLegacyPreferences(key => localStorage.getItem(key)), setView, requestPreferences));
  useEffect(() => {
    const refresh = () => { if (!document.hidden) void sync.refresh().catch(() => {}); };
    void sync.refresh().catch(() => {});
    const timer = window.setInterval(refresh, 5000);
    window.addEventListener('focus', refresh);
    window.addEventListener('online', refresh);
    document.addEventListener('visibilitychange', refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener('focus', refresh);
      window.removeEventListener('online', refresh);
      document.removeEventListener('visibilitychange', refresh);
    };
  }, [sync]);
  return [view ?? sync.current(), sync] as const;
}

export function setIdEnabled(ids: string[], id: string, enabled: boolean) {
  return enabled ? ids.includes(id) ? ids : [...ids, id] : ids.filter(item => item !== id);
}
