export const MOBILE_NAV_STORAGE_KEY = 'yahu.mobile-nav.v2';
export const LEGACY_MOBILE_NAV_STORAGE_KEY = 'yahu.mobile-nav.v1';
export const MOBILE_NAV_LIMIT = 7;

export const MOBILE_NAV_MODES = [
  'chat',
  'cron',
  'skills',
  'insights',
  'usage',
  'images',
  'memory',
  'workspace',
  'terminal',
] as const;

export type MobileNavMode = typeof MOBILE_NAV_MODES[number];

export const DEFAULT_MOBILE_NAV_MODES: readonly MobileNavMode[] = ['chat', 'cron', 'skills', 'insights', 'usage', 'workspace', 'terminal'];
const LEGACY_DEFAULT_MOBILE_NAV_MODES: readonly MobileNavMode[] = ['chat', 'cron', 'skills', 'insights', 'usage', 'terminal'];

function sameModes(left: readonly MobileNavMode[], right: readonly MobileNavMode[]) {
  return left.length === right.length && left.every((mode, index) => mode === right[index]);
}

export function normalizeMobileNavModes(value: unknown): MobileNavMode[] {
  if (!Array.isArray(value)) return [...DEFAULT_MOBILE_NAV_MODES];
  const allowed = new Set<string>(MOBILE_NAV_MODES);
  const result: MobileNavMode[] = [];
  for (const item of value) {
    if (typeof item !== 'string' || !allowed.has(item) || result.includes(item as MobileNavMode)) continue;
    result.push(item as MobileNavMode);
    if (result.length === MOBILE_NAV_LIMIT) break;
  }
  return result;
}

export function readMobileNavModes(storage: Pick<Storage, 'getItem'>): MobileNavMode[] {
  const stored = storage.getItem(MOBILE_NAV_STORAGE_KEY);
  if (stored !== null) {
    try {
      return normalizeMobileNavModes(JSON.parse(stored));
    } catch {
      return [...DEFAULT_MOBILE_NAV_MODES];
    }
  }

  const legacyStored = storage.getItem(LEGACY_MOBILE_NAV_STORAGE_KEY);
  if (legacyStored === null) return [...DEFAULT_MOBILE_NAV_MODES];
  try {
    const legacyModes = normalizeMobileNavModes(JSON.parse(legacyStored));
    return sameModes(legacyModes, LEGACY_DEFAULT_MOBILE_NAV_MODES) ? [...DEFAULT_MOBILE_NAV_MODES] : legacyModes;
  } catch {
    return [...DEFAULT_MOBILE_NAV_MODES];
  }
}
