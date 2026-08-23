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
