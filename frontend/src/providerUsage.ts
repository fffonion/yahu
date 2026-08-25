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

export type ProviderUsageSection = {
  provider: string;
  title: string;
  description: string;
  captured_at?: number;
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

export function sectionHasContent(section: ProviderUsageSection): boolean {
  return section.rows.length > 0 || section.windows.length > 0 || section.description.length > 0;
}

export function providerCodexResetSubtitle(description: string | undefined): string {
  const text = (description || '').trim();
  if (!text) return '';
  const parts: string[] = [];
  const pattern = /([^；]+)：Reset：(\d+)个(?:；当前可用：\d+个)?(?:；到期：([^；]+))?/g;
  for (const match of text.matchAll(pattern)) {
    const count = Number(match[2]);
    if (!Number.isFinite(count) || count <= 0) continue;
    const expiry = match[3]?.trim();
    parts.push(`${match[1].trim()}：${count}个重置${expiry ? ` ${expiry}到期` : ''}`);
  }
  return parts.join('；');
}
