export type ToolField = { key: string; value: string };
export type ToolSummary = { title: string; toolName: string; subtitle: string; fields: ToolField[]; raw: unknown; status: string };

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

export function tryParseJson(text: string): unknown {
  try { return JSON.parse(text); }
  catch { return text; }
}

function decodeXmlEntities(text: string): string {
  return text
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');
}

function parseUntrustedToolResult(text: string): Record<string, unknown> | null {
  const match = text.match(/^\s*<untrusted_tool_result\b([^>]*)>([\s\S]*?)<\/untrusted_tool_result>\s*$/);
  if (!match) return null;
  const attrs: Record<string, string> = {};
  for (const attr of match[1].matchAll(/([:\w-]+)\s*=\s*"([^"]*)"/g)) {
    attrs[attr[1]] = decodeXmlEntities(attr[2]);
  }
  return { type: 'untrusted_tool_result', ...attrs, result: decodeXmlEntities(match[2].trim()) };
}

export function toolDisplayName(name: string): string {
  return name.replace(/^functions\./, '').replace(/_/g, ' ');
}

export function summarizeToolMessage(content: string): ToolSummary {
  const parsed = parseUntrustedToolResult(content) ?? tryParseJson(content);
  const root = asRecord(parsed);
  const usesSourceName = typeof root?.source === 'string' && root.source;
  const toolName = String(root?.source || root?.tool_name || root?.name || root?.tool || root?.recipient_name || root?.function || 'tool');
  const rawStatus = root?.status;
  const status = (() => {
    if (root?.success === false) return 'error';
    if (typeof root?.error === 'string' && root.error) return 'error';
    if (root?.exit_code !== undefined && root?.exit_code !== null && Number(root.exit_code) !== 0) return 'error';
    const s = String(rawStatus || 'ok').toLowerCase();
    if (s === 'error' || s === 'failed' || s.startsWith('4') || s.startsWith('5')) return s;
    return 'ok';
  })();
  const title = usesSourceName ? toolName : toolDisplayName(toolName);
  const fullTitle = status !== 'ok' ? `${title} · ${status}` : title;
  const fields: ToolField[] = [];

  const push = (key: string, value: unknown) => {
    if (value === undefined || value === null || value === '') return;
    const text = typeof value === 'string' ? value : JSON.stringify(value);
    fields.push({ key, value: text.length > 500 ? `${text.slice(0, 500)}…` : text });
  };

  if (root) {
    push('source', root.source);
    push('result', root.result ?? root.output ?? root.message ?? root.content);
    push('error', root.error);
    push('path', root.path ?? root.file ?? root.resolved_path);
    push('status', root.status);
    push('exit code', root.exit_code);
    const data = asRecord(root.data);
    if (data) Object.entries(data).slice(0, 8).forEach(([k, v]) => push(k, v));
  } else {
    push('output', content);
  }

  const subtitle = fields.find((f) => f.key === 'error')?.value
    || fields.find((f) => ['result', 'output', 'message', 'content'].includes(f.key))?.value
    || (root ? `${Object.keys(root).length} fields` : content);

  return { title: fullTitle, toolName, subtitle: subtitle.replace(/\s+/g, ' ').slice(0, 180), fields, raw: parsed, status };
}
