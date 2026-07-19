export type SearchFilesMatch = {
  lineNumber: string;
  content: string;
  isMatch: boolean;
};

export type SearchFilesGroup = {
  path: string;
  matches: SearchFilesMatch[];
};

export type ParsedSearchFilesResult = {
  totalCount: number;
  groups: SearchFilesGroup[];
  files: string[];
  error: string;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function parseJsonObject(value: unknown): Record<string, unknown> | null {
  if (typeof value !== 'string') return asRecord(value);
  const text = value.trim();
  if (!text.startsWith('{')) return null;
  try { return asRecord(JSON.parse(text)); }
  catch { return null; }
}

function parseGroupedMatches(text: string): SearchFilesGroup[] {
  const groups: SearchFilesGroup[] = [];
  let current: SearchFilesGroup | null = null;

  for (const rawLine of text.split('\n')) {
    if (!rawLine.trim()) continue;
    const row = rawLine.match(/^\s*(\d+)([:\-])\s?(.*)$/);
    if (row && current) {
      current.matches.push({ lineNumber: row[1], content: row[3], isMatch: row[2] === ':' });
      continue;
    }
    current = { path: rawLine.trim(), matches: [] };
    groups.push(current);
  }

  return groups.filter((group) => group.path && group.matches.length > 0);
}

export function parseSearchFilesResult(value: unknown): ParsedSearchFilesResult | null {
  const record = parseJsonObject(value);
  if (!record) return null;
  const recognized = ['total_count', 'matches_text', 'matches', 'error'].some((key) => Object.prototype.hasOwnProperty.call(record, key));
  if (!recognized) return null;

  const groups = typeof record.matches_text === 'string' ? parseGroupedMatches(record.matches_text) : [];
  const files = Array.isArray(record.matches) ? record.matches.filter((item): item is string => typeof item === 'string' && !!item.trim()).map((item) => item.trim()) : [];
  const explicitTotal = Number(record.total_count);
  const inferredTotal = groups.reduce((sum, group) => sum + group.matches.filter((match) => match.isMatch).length, 0) + files.length;

  return {
    totalCount: Number.isFinite(explicitTotal) && explicitTotal >= 0 ? explicitTotal : inferredTotal,
    groups,
    files,
    error: typeof record.error === 'string' ? record.error : '',
  };
}
