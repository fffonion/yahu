import { expect, test } from 'bun:test';
import { createPreferenceSync, readLegacyPreferences } from './uiPreferences';

test('imports existing legacy preferences without filling or overwriting initialized sections', async () => {
  const local = new Map([['pinnedSessions', '["b","a","b"]'], ['yahu.provider-usage.enabled.v1', '{"codex":true,"grok":false}']]);
  const legacy = readLegacyPreferences(key => local.get(key) ?? null);
  expect(legacy).toEqual({ pinned_sessions: ['b', 'a'], enabled_providers: ['codex'] });
  const state = { revision: 4, pinned_sessions: [], enabled_providers: null, provider_order: ['grok'] };
  const sync = createPreferenceSync(legacy, () => {}, async (patch) => {
    if (!patch) return { preferences: state, conflict: false };
    expect(patch).toEqual({ revision: 4, enabled_providers: ['codex'] });
    return { preferences: { ...state, revision: 5, enabled_providers: ['codex'] }, conflict: false };
  });
  await sync.refresh();
  expect(sync.current().pinned_sessions).toEqual([]);
  expect(sync.current().enabled_providers).toEqual(['codex']);
});

test('replays local intent over remote writes and does not replace other preference fields', async () => {
  let remote = { revision: 1, pinned_sessions: ['old'], enabled_providers: ['codex'], provider_order: ['codex'] };
  const sync = createPreferenceSync({}, () => {}, async (patch) => {
    if (!patch) return { preferences: remote, conflict: false };
    if (patch.revision !== remote.revision) return { preferences: remote, conflict: true };
    remote = { ...remote, ...patch, revision: remote.revision + 1 };
    return { preferences: remote, conflict: false };
  });
  await sync.refresh();
  remote = { ...remote, revision: 2, pinned_sessions: ['from-phone', 'old'], provider_order: ['grok', 'codex'] };
  await sync.change('pinned_sessions', ids => [...ids, 'from-desktop']);
  expect(remote.pinned_sessions).toEqual(['from-phone', 'old', 'from-desktop']);
  expect(sync.current().provider_order).toEqual(['grok', 'codex']);
});
