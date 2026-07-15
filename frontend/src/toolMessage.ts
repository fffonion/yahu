export type ToolField = { key: string; value: string };
export type ToolSummary = { title: string; toolName: string; subtitle: string; fields: ToolField[]; raw: unknown; input?: unknown; result: unknown; status: string };

const INPUT_KEYS = ['arguments', 'args', 'params', 'parameters', 'input', 'tool_input', 'tool_args', 'request'];
const META_KEYS = new Set(['source', 'tool_name', 'name', 'tool', 'recipient_name', 'function', 'status', 'success', ...INPUT_KEYS]);

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

function cleanToolName(value: unknown): string {
  return typeof value === 'string' && value.trim() ? value.trim() : '';
}

function parseMaybeJson(value: unknown): unknown {
  if (typeof value !== 'string') return value;
  const trimmed = value.trim();
  if (!trimmed || !/^[\[{]/.test(trimmed)) return value;
  try { return JSON.parse(trimmed); } catch { return value; }
}

function firstDefined(record: Record<string, unknown>, keys: string[]): unknown {
  for (const key of keys) {
    const value = record[key];
    if (value !== undefined && value !== null && value !== '') return parseMaybeJson(value);
  }
  return undefined;
}

function invocationFromRecord(root: Record<string, unknown> | null): unknown {
  if (!root) return undefined;
  const direct = firstDefined(root, INPUT_KEYS);
  if (direct !== undefined) return direct;
  const toolCall = asRecord(root.tool_call);
  const nestedToolCall = firstDefined(toolCall || {}, INPUT_KEYS);
  if (nestedToolCall !== undefined) return nestedToolCall;
  const fn = asRecord(root.function);
  return firstDefined(fn || {}, ['arguments', 'args', 'params', 'parameters']);
}

function resultFromRecord(root: Record<string, unknown> | null, parsed: unknown): unknown {
  if (!root) return parsed;
  const primary = firstDefined(root, ['result', 'output', 'message', 'content', 'data', 'error']);
  if (primary !== undefined) return primary;
  const entries = Object.entries(root).filter(([key]) => !META_KEYS.has(key));
  if (!entries.length) return parsed;
  if (entries.length === 1) return entries[0][1];
  return Object.fromEntries(entries);
}

function commandFromInput(toolName: string, input: unknown): string {
  const name = toolName.replace(/^functions\./, '');
  const record = asRecord(input);
  const command = record?.command;
  return name === 'terminal' && typeof command === 'string' && command.trim() ? command.trim() : '';
}

function patchPathFromInput(root: Record<string, unknown> | null, input: unknown): string {
  const invocation = asRecord(input);
  const files = Array.isArray(root?.files_modified) ? root.files_modified : [];
  const value = invocation?.path ?? root?.resolved_path ?? root?.path ?? root?.file ?? files[0];
  if (typeof value !== 'string' || !value.trim()) return '';
  const normalized = value.trim().replace(/\\/g, '/').replace(/\/+$/, '');
  const parts = normalized.split('/').filter(Boolean);
  if (parts.length < 2) return normalized;
  const filename = parts[parts.length - 1];
  const srcIndex = parts.indexOf('src');
  if (srcIndex < 0) return filename;
  const relevant = parts.slice(srcIndex);
  const dirs = relevant.slice(0, -1);
  if (dirs.length <= 2) return relevant.join('/');
  return `${dirs[0]}/…/${dirs[dirs.length - 1]}/${filename}`;
}

export function summarizeToolMessage(content: string, fallbackToolName = '', fallbackInput?: unknown): ToolSummary {
  const parsed = parseUntrustedToolResult(content) ?? tryParseJson(content);
  const root = asRecord(parsed);
  const fallbackName = cleanToolName(fallbackToolName);
  const explicitContentToolName = cleanToolName(root?.source) || cleanToolName(root?.tool_name) || cleanToolName(root?.tool) || cleanToolName(root?.recipient_name) || cleanToolName(root?.function);
  const contentToolName = explicitContentToolName || (!fallbackName ? cleanToolName(root?.name) : '');
  const toolName = contentToolName || fallbackName || 'tool';
  const usesSourceName = !!cleanToolName(root?.source);
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
    push('input', invocationFromRecord(root));
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

  const input = invocationFromRecord(root) ?? parseMaybeJson(fallbackInput);
  const command = commandFromInput(toolName, input);
  const errorSubtitle = fields.find((f) => f.key === 'error')?.value;
  const resultSubtitle = fields.find((f) => ['result', 'output', 'message', 'content'].includes(f.key))?.value;
  const patchPath = toolName === 'patch' || toolName === 'functions.patch' || contentToolName === 'patch' ? patchPathFromInput(root, input) : '';
  const subtitle = command
    || errorSubtitle
    || resultSubtitle
    || (root ? `${Object.keys(root).length} fields` : content);

  const decoratedSubtitle = patchPath && errorSubtitle ? subtitle
    : patchPath && subtitle !== patchPath ? `${patchPath} · ${subtitle}`
    : patchPath && root ? `${patchPath} · ${Object.keys(root).length} fields`
    : subtitle;

  return { title: fullTitle, toolName, subtitle: decoratedSubtitle.replace(/\s+/g, ' ').slice(0, 180), fields, raw: parsed, input, result: resultFromRecord(root, parsed), status };
}
