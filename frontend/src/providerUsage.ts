export type ProviderUsageRow = {
  label: string;
  hit_rate?: string | null;
  input?: string | null;
  output?: string | null;
  cost_or_pct?: string | null;
};

export type ProviderUsageWindow = {
  window: string;
  used?: string | null;
  reset?: string | null;
  reset_at?: number | null;
};

export type ProviderUsageCredit = { account: string; balance: number };

export type ProviderUsageSection = {
  provider: string;
  title: string;
  description: string;
  captured_at?: number;
  credits?: ProviderUsageCredit[];
  rows: ProviderUsageRow[];
  windows: ProviderUsageWindow[];
  errors: string[];
};

export interface ProviderUsageProvider {
  provider: string;
  title: string;
  configured: boolean;
  query_ready: boolean;
  credential_hint: string;
  setup_hint: string;
}

export interface ProviderUsagePayload {
  fetched_at: number;
  providers: ProviderUsageProvider[];
  sections: ProviderUsageSection[];
}

export type ProviderUsageAccountGroup = [account: string, windows: ProviderUsageWindow[]];

export function providerUsagePercent(value: string | null | undefined): number | null {
  const match = value?.match(/(-?\d+(?:\.\d+)?)\s*%/);
  if (match) {
    const percent = Number(match[1]);
    return Number.isFinite(percent) ? Math.max(0, Math.min(100, percent)) : null;
  }
  const ratio = value?.match(/([\d,.]+)\s*\/\s*([\d,.]+)/);
  if (!ratio) return null;
  const used = Number(ratio[1].replace(/,/g, ''));
  const limit = Number(ratio[2].replace(/,/g, ''));
  return Number.isFinite(used) && Number.isFinite(limit) && limit > 0 ? Math.max(0, Math.min(100, used / limit * 100)) : null;
}

export function providerUsageAccountHasActiveQuotaWall(
  windows: ProviderUsageWindow[],
  now = Date.now() / 1000,
  quotaWallPercent = 100,
): boolean {
  return windows.some((window) => {
    const percent = providerUsagePercent(window.used);
    return percent !== null && percent >= quotaWallPercent && typeof window.reset_at === 'number' && window.reset_at > now;
  });
}

export function orderProviderUsageAccountGroups(groups: ProviderUsageAccountGroup[], quotaWallPercent = 100): ProviderUsageAccountGroup[] {
  return groups
    .map(([account, windows], index) => ({
      account,
      windows,
      index,
      hasQuotaWall: windows.some((window) => (providerUsagePercent(window.used) ?? -1) >= quotaWallPercent),
    }))
    .sort((left, right) => Number(left.hasQuotaWall) - Number(right.hasQuotaWall) || left.index - right.index)
    .map(({ account, windows }) => [account, windows]);
}

export function orderNewApiUsageRows(provider: string, rows: ProviderUsageRow[]): ProviderUsageRow[] {
  if (!['agentrouter', 'justwoker', 'hcnsec'].includes(provider)) return rows;
  const tokenUsage = (row: ProviderUsageRow) => {
    const value = Number((row.input || '').replace(/,/g, ''));
    return Number.isFinite(value) && value > 0 ? value : 0;
  };
  return [...rows].sort((left, right) => tokenUsage(right) - tokenUsage(left));
}

export function providerVyceCalls(description: string | undefined): string {
  return description?.match(/(?:^|[；;])\s*(?:今日)?调用次数\s+(\d[\d,]*)(?=\s*(?:·|$))/)?.[1] || '';
}

function codexCreditEntries(credits: ProviderUsageCredit[] | undefined): [string, string][] {
  const positive = (credits || []).filter((credit) => Number.isFinite(credit.balance) && Math.trunc(credit.balance) > 0);
  if (!positive.length) return [];
  const formatter = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0, useGrouping: false });
  return positive.map(({ account, balance }) => [account, `Credit ${formatter.format(Math.trunc(balance))}`]);
}

export function providerCodexCreditSubtitle(credits: ProviderUsageCredit[] | undefined): string {
  return codexCreditEntries(credits).map(([account, detail]) => `${account}：${detail}`).join('；');
}

export function sectionHasContent(section: ProviderUsageSection): boolean {
  return section.rows.length > 0 || section.windows.length > 0 || section.description.length > 0
    || Boolean(section.credits?.some((credit) => Number.isFinite(credit.balance) && credit.balance > 0));
}

type CodexResetEntry = { account: string; count: number; expiry: string };

function codexResetEntries(description: string | undefined): CodexResetEntry[] {
  const text = (description || '').trim();
  if (!text) return [];
  const pattern = /([^；]+)：Reset：(\d+)个(?:；当前可用：\d+个)?(?:；到期：([^；]+))?/g;
  return Array.from(text.matchAll(pattern), (match) => ({
    account: match[1].trim(),
    count: Number(match[2]),
    expiry: match[3]?.trim() || '',
  })).filter((entry) => Number.isFinite(entry.count));
}

function codexResetDurations(expiry: string): string[] {
  return expiry.split(/[、,，]/)
    .map((value) => value.trim().replace(/后$/, ''))
    .filter(Boolean);
}

function codexAccountSubtitle(description: string | undefined, credits: ProviderUsageCredit[] | undefined, mobile: boolean): string {
  const groups = new Map<string, string[]>();
  for (const [account, detail] of codexCreditEntries(credits)) groups.set(account, [detail]);
  for (const entry of codexResetEntries(description)) {
    if (entry.count <= 0) continue;
    const detail = mobile ? codexResetDurations(entry.expiry).join(', ')
      : `${entry.count}个重置${entry.expiry ? ` ${entry.expiry}到期` : ''}`;
    if (detail) groups.set(entry.account, [...(groups.get(entry.account) || []), detail]);
  }
  return [...groups].map(([account, details]) =>
    `${account}${mobile && !details[0].startsWith('Credit ') ? ': ' : '：'}${details.join(' · ')}`
  ).join(mobile ? '; ' : '；');
}

export function providerCodexResetSubtitle(description: string | undefined, credits?: ProviderUsageCredit[]): string {
  return codexAccountSubtitle(description, credits, false);
}

export function providerCodexMobileResetSubtitle(description: string | undefined, credits?: ProviderUsageCredit[]): string {
  return codexAccountSubtitle(description, credits, true);
}
