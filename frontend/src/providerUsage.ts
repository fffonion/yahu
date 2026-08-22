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
};

export type ProviderUsageSection = {
  provider: string;
  title: string;
  description: string;
  rows: ProviderUsageRow[];
  windows: ProviderUsageWindow[];
  errors: string[];
};

export type ProviderUsagePayload = {
  fetched_at: number;
  sections: ProviderUsageSection[];
};

export function sectionHasContent(section: ProviderUsageSection): boolean {
  return section.rows.length > 0 || section.windows.length > 0 || section.description.length > 0;
}
