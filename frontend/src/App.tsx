import React, { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { Bot, Brain, CalendarClock, CheckSquare, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Circle as SelectionMark, CircleHelp, Code, Download, Eye, FileText, Folder, Globe, GripVertical, History, Home, Image as ImageIcon, Info, Layout, Lightbulb, LineChart, List, MessageSquare, Network, Palette, Paperclip, Pencil, Pin, PinOff, PlayCircle as PlayMark, Plus, Puzzle, RefreshCw, Repeat, Save, Search, Send, Server, Settings, Star, Terminal, Trash2, UserRound, Users, Video, Volume2, X } from 'lucide-react';
import { buildChatInputWithAttachments } from './attachmentPayload';
import { buildChatRequestBody } from './chatRequest';
import { buildCronPatch, cronEditableValues } from './cronEditor';
import { createStreamAnimator } from './streamAnimator';
import { currentModelDisplayOption, providerDisplayName } from './modelDisplay';
import { summarizeToolMessage } from './toolMessage';
import { sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';
import { buildHashRoute, getCurrentHashRoute, type HashRoute } from './hashRoute';
import { areaPath, chartPoint, chartTooltipLabel, chartYAxisTicks, emptyTotals, finalizeTotals, fmtCompactAxisTick, fmtMoney, fmtPercent, fmtTokens, formatMetricValue, linePath, metricLabels, metricValue, modelPeriodTotals, periodSlice, type UsageDay, type UsageInsights, type UsageMetric, type UsageModel, type UsageSource, type UsageTotals } from './insights';
import { normalizeMessageParts } from './messageReasoning';
import { shouldRenderMessage } from './messageVisibility';
import { initLang, setLang as setI18nLang, getLang, t, type Lang } from './i18n';

type Theme = 'hermes-light' | 'hermes-dark' | 'vscode-light-plus' | 'vscode-dark-plus' | 'monokai' | 'nord' | 'solarized-dark' | 'catppuccin-latte' | 'catppuccin-mocha' | 'nous';
type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'images' | 'workspace' | 'skills' | 'settings';
type Role = 'user' | 'assistant' | 'system' | 'tool';
type FollowUpBehaviour = 'queue' | 'steer';
type ComposerEnterMode = 'enter-send' | 'enter-newline';
type Session = { id: string; source?: string; title?: string; preview?: string; started_at?: number | string; ended_at?: number | string; last_active?: number | string; message_count?: number; input_tokens?: number; output_tokens?: number; model?: string; provider?: string };
type ChatMessage = { id: string; role: Role; content: string; reasoning?: string; timestamp?: string | number; pending?: boolean; toolName?: string; toolInput?: unknown };
type FollowUpQueueItem = { id: string; text: string; createdAt: number };
type ModelOption = { id: string; label: string; provider?: string };
type Attachment = { id: string; name: string; kind: 'image' | 'text' | 'binary'; mime: string; size: number; dataUrl?: string; text?: string; uploadedPath?: string };
type SessionContextMenu = { session: Session; x: number; y: number } | null;
type WorkspaceEntry = { name: string; path: string; kind: 'file' | 'dir'; size?: number; modified?: string };
type WorkspacePreview = { path: string; content: string; kind: 'text' | 'image' | 'none'; url?: string; editRequest?: number };
type Skill = { name: string; description?: string; category?: string; enabled?: boolean };
type WorkspaceContextMenu = { entry: WorkspaceEntry; x: number; y: number } | null;
type DialogState = { variant: 'prompt' | 'confirm'; title: string; message: string; value?: string; danger?: boolean; resolve: (value: any) => void } | null;
type Job = { job_id?: string; id?: string; name?: string; schedule?: string | { display?: string; expr?: string }; prompt?: string; script?: string | null; status?: string; paused?: boolean; enabled?: boolean; next_run?: string; last_run?: string; deliver?: string };
type MemoryDoc = { memory: string; user: string };
type ImageEntry = { filename: string; heic_filename?: string | null; image_url: string; png_url: string; heic_url?: string | null; heic_status: 'available' | 'missing' | 'not_applicable' | string; download_filename: string; download_url: string; download_label: string; created_at: number; modified_at: number; size: number };
type ImageStats = { total_images: number; total_bytes: number };
type ImageMetadata = { filename: string; dimensions?: { width: number; height: number } | null; png: { filename: string; url: string; size: number; modified_at: number }; webp?: unknown; heic?: unknown; heic_status: string; png_text: Array<{ keyword: string; value: string }> };

type MessagePage = { data: any[]; total: number; has_older: boolean; has_newer: boolean };

const DEFAULT_API_BASE = '/hermes';
const DRAFT_SESSION_ID = '__webui_draft_session__';
const FOLLOW_UP_BEHAVIOUR_KEY = 'followUpBehaviour';
const FOLLOW_UP_QUEUES_KEY = 'followUpQueues';
const COMPOSER_ENTER_MODE_KEY = 'composerEnterMode';
const THEME_OPTIONS: Array<{ id: Theme; label: string }> = [
  { id: 'hermes-light', label: 'Hermes Light' },
  { id: 'hermes-dark', label: 'Hermes Dark' },
  { id: 'vscode-light-plus', label: 'VS Code Light+' },
  { id: 'vscode-dark-plus', label: 'VS Code Dark+' },
  { id: 'monokai', label: 'Monokai' },
  { id: 'nord', label: 'Nord' },
  { id: 'solarized-dark', label: 'Solarized Dark' },
  { id: 'catppuccin-latte', label: 'Catppuccin Latte' },
  { id: 'catppuccin-mocha', label: 'Catppuccin Mocha' },
  { id: 'nous', label: 'Nous' },
];
const DARK_THEMES = new Set<Theme>(['hermes-dark', 'vscode-dark-plus', 'monokai', 'nord', 'solarized-dark', 'catppuccin-mocha', 'nous']);
const normalizeTheme = (value: string | null): Theme => {
  if (value === 'dark') return 'hermes-dark';
  if (value === 'light') return 'hermes-light';
  return THEME_OPTIONS.some((item) => item.id === value) ? value as Theme : 'hermes-dark';
};
const isDarkTheme = (value: Theme) => DARK_THEMES.has(value);
const themeLabel = (value: Theme) => THEME_OPTIONS.find((item) => item.id === value)?.label || value;
const normalizeFollowUpBehaviour = (value: string | null): FollowUpBehaviour => value === 'steer' ? 'steer' : 'queue';
const normalizeComposerEnterMode = (value: string | null): ComposerEnterMode => value === 'enter-newline' ? 'enter-newline' : 'enter-send';
const readFollowUpQueues = (): Record<string, FollowUpQueueItem[]> => {
  try {
    const parsed = JSON.parse(localStorage.getItem(FOLLOW_UP_QUEUES_KEY) || '{}');
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).map(([key, value]) => [key, Array.isArray(value) ? value.filter((item: any) => item && typeof item.text === 'string').map((item: any) => ({ id: String(item.id || uid('fu')), text: item.text, createdAt: Number(item.createdAt || Date.now()) })) : []]));
  } catch { return {}; }
};
const followUpQueueKey = (sessionId: string) => sessionId || DRAFT_SESSION_ID;
const EFFORTS = ['minimal', 'low', 'medium', 'high'] as const;
const hasMobileDrawer = (mode: Mode) => mode === 'chat' || mode === 'cron' || mode === 'workspace' || mode === 'skills';
const MESSAGE_PAGE = 24;
const MESSAGE_WINDOW = 120;
const OTHER_PLATFORM_PENDING_ID = 'other-platform-pending';
const initialRoute = getCurrentHashRoute();

const uid = (prefix: string) => `${prefix}_${Date.now().toString(36)}_${Math.random().toString(16).slice(2)}`;
const fmtSize = (bytes?: number) => bytes === undefined ? '' : bytes < 1024 ? `${bytes} B` : bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(1)} k` : `${(bytes / 1024 / 1024).toFixed(1)} M`;
const basename = (path: string) => path.split('/').filter(Boolean).pop() || 'Home';

function useMediaQuery(query: string) {
  const read = () => typeof window !== 'undefined' && window.matchMedia(query).matches;
  const [matches, setMatches] = useState(read);
  useEffect(() => {
    if (typeof window === 'undefined') return;
    const media = window.matchMedia(query);
    const update = () => setMatches(media.matches);
    update();
    media.addEventListener('change', update);
    return () => media.removeEventListener('change', update);
  }, [query]);
  return matches;
}
const isWorkspaceTextFile = (name: string) => /\.(md|txt|json|ya?ml|toml|csv|ts|tsx|js|jsx|py|rs|go|sh|css|html|lock)$/i.test(name) || /^(Makefile|Dockerfile|\.gitignore|\.dockerignore|\.env|\.npmrc|\.prettierrc|\.eslintrc)$/i.test(name);
const jobId = (job: Job) => job.job_id || job.id || '';
const jobSchedule = (schedule: Job['schedule']) => typeof schedule === 'string' ? schedule : (schedule?.display || schedule?.expr || 'no schedule');
const jobState = (job: Job) => job.status || (job.paused || job.enabled === false ? 'paused' : 'active');
const apiJoin = (base: string, path: string) => `${base.replace(/\/$/, '')}${path}`;
const numericId = (id?: string) => /^\d+$/.test(id || '') ? id : '';
const workspaceRouteParents = (path: string) => {
  const parts = path.split('/').filter(Boolean);
  const parents: string[] = [''];
  for (let i = 1; i < parts.length; i += 1) parents.push(parts.slice(0, i).join('/'));
  return parents;
};

async function readFileAttachment(file: File): Promise<Attachment> {
  const id = uid('att');
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
  if (file.type.startsWith('image/')) {
    return { id, name: file.name, kind: 'image', mime: file.type, size: file.size, dataUrl };
  }
  const isText = file.type.startsWith('text/') || /\.(md|txt|json|ya?ml|toml|csv|ts|tsx|js|jsx|py|rs|go|sh|css|html)$/i.test(file.name);
  if (isText && file.size <= 256 * 1024) return { id, name: file.name, kind: 'text', mime: file.type || 'text/plain', size: file.size, dataUrl, text: await file.text() };
  return { id, name: file.name, kind: 'binary', mime: file.type || 'application/octet-stream', size: file.size, dataUrl };
}
function parseSseBlock(block: string) {
  let event = 'message';
  const data: string[] = [];
  for (const line of block.split('\n')) {
    if (line.startsWith('event:')) event = line.slice(6).trim();
    if (line.startsWith('data:')) data.push(line.slice(5).trimStart());
  }
  return { event, data: data.join('\n') };
}
function roleName(role: Role) { return role === 'assistant' ? 'Hermes Agent' : role === 'tool' ? 'Tool' : role === 'system' ? 'System' : 'You'; }
function markdownText(text: string) {
  const escaped = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return escaped
    .replace(/```([\s\S]*?)```/g, '<pre><code>$1</code></pre>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<span class="md-strong">$1</span>')
    .replace(/\n/g, '<br/>');
}
function escapeHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
function highlightWorkspaceText(text: string, filePath?: string) {
  const ext = (filePath || '').split('.').pop()?.toLowerCase();
  const language = (() => {
    switch (ext) {
      case 'rs': return 'rust';
      case 'py': return 'python';
      case 'ts': case 'tsx': case 'js': case 'jsx': return 'javascript';
      case 'css': return 'css';
      case 'html': case 'htm': return 'html';
      case 'json': return 'json';
      case 'md': return 'markdown';
      case 'toml': case 'yaml': case 'yml': return 'config';
      case 'sh': case 'bash': return 'shell';
      case 'go': return 'go';
      case 'c': case 'h': return 'c';
      case 'cpp': case 'hpp': case 'cc': case 'cxx': return 'cpp';
      case 'java': return 'java';
      case 'rb': return 'ruby';
      case 'sql': return 'sql';
      case 'xml': case 'svg': return 'xml';
      default: return 'plain';
    }
  })();

  const patterns: Record<string, RegExp> = {
    rust: /\/\/[^\n]*|\/\/!.*|"(?:[^"\\]|\\.)*"|r#"(?:[^"]|"(?!\n#))*"#|r"(?:[^"]|"(?!\n))*"|'(?:[^'\\]|\\.)'|b'(?:[^'\\])'|\b(?:fn|let|mut|pub|struct|impl|trait|enum|match|if|else|for|while|loop|return|use|mod|const|static|type|where|as|in|ref|move|unsafe|extern|crate|self|super|true|false|Some|None|Ok|Err)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?(?:_?\d+)*(?:[uUiIfF](?:8|16|32|64|128|size)?)?\b/g,
    python: /#[^\n]*|"""["\n]*?"""|'''['\n]*?'''|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:def|class|import|from|return|if|elif|else|for|while|try|except|finally|raise|with|as|in|is|not|and|or|lambda|yield|async|await|pass|break|continue|True|False|None)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
    javascript: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|`(?:[^`\\]|\\.)*`|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:const|let|var|function|return|async|await|import|export|from|type|interface|if|else|for|while|class|extends|new|true|false|null|undefined|try|catch|throw|switch|case|default|break|continue|of|in|typeof)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
    css: /\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|#[0-9a-fA-F]{3,8}\b|\b\d+(?:\.\d+)?(?:px|em|rem|%|vh|vw|pt|ms|s|deg)?\b|[\w-]+(?=\s*:)|[:;{},]/g,
    html: /<!--[\s\S]*?-->|<[\/!]?\w[\w-]*(?:\s[^>]*)?>|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g,
    json: /"(?:[^"\\]|\\.)*"\s*:/g,
    markdown: /^#{1,6}\s.*$|`[^`]+`|```[\s\S]*?```|\*\*[^*]+\*\*|__[^_]+__|\[[^\]]+\]\([^)]+\)/gm,
    config: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:true|false|null|yes|no|on|off)\b|\b\d+(?:\.\d+)?\b/g,
    shell: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|in|function|return|exit|echo|export|source|local|readonly|declare)\b|\b\d+\b|\$\{?[\w_]+\}?/g,
    go: /\/\/[^\n]*|"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`|'(?:[^'\\])'|\b(?:func|return|if|else|for|range|switch|case|default|break|continue|go|defer|chan|select|map|struct|interface|type|var|const|package|import|nil|true|false)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
    c: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:if|else|for|while|do|switch|case|default|break|continue|return|struct|typedef|enum|union|static|extern|const|volatile|sizeof|void|int|char|float|double|long|short|unsigned|signed)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFlLuU]?\b/g,
    cpp: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:class|struct|enum|namespace|using|template|typename|virtual|override|public|private|protected|const|constexpr|auto|decltype|static_cast|dynamic_cast|nullptr|new|delete|try|catch|throw|if|else|for|while|do|switch|case|default|break|continue|return|true|false)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFlLuU]?\b/g,
    java: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:public|private|protected|static|final|class|interface|extends|implements|abstract|synchronized|volatile|transient|native|strictfp|void|int|long|double|float|boolean|char|byte|short|new|return|if|else|for|while|do|switch|case|default|break|continue|try|catch|throw|throws|import|package|true|false|null|this|super)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFdDlL]?\b/g,
    ruby: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|:(?:\w+[?!]?)|:\s*"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`|\b(?:def|end|class|module|if|elsif|else|unless|while|until|for|do|begin|rescue|ensure|case|when|return|yield|self|nil|true|false|and|or|not|require|include|extend|attr_accessor|attr_reader|attr_writer|private|protected|public)\b|\b\d+(?:\.\d+)?\b/g,
    sql: /--[^\n]*|\/\*[\s\S]*?\*\/|'(?:[^'\\]|\\.)*'|\b(?:SELECT|FROM|WHERE|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|TABLE|INDEX|VIEW|JOIN|LEFT|RIGHT|INNER|OUTER|ON|AS|AND|OR|NOT|IN|BETWEEN|LIKE|IS|NULL|ORDER|BY|GROUP|HAVING|LIMIT|OFFSET|UNION|ALL|DISTINCT|COUNT|SUM|AVG|MIN|MAX|CASE|WHEN|THEN|ELSE|END|SET|VALUES|INTO|PRIMARY|KEY|FOREIGN|REFERENCES|CONSTRAINT|DEFAULT|CHECK|UNIQUE|EXISTS)\b|\b\d+(?:\.\d+)?\b/gi,
    xml: /<!--[\s\S]*?-->|<[\/!]?\w[\w:.-]*(?:\s[^>]*)?>|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g,
  };

  const token = patterns[language] || patterns.javascript;
  if (language === 'plain' && !/[\w#"'/]/.test(text.slice(0, 200))) return escapeHtml(text);

  let out = '';
  let last = 0;
  for (const match of text.matchAll(token)) {
    const part = match[0];
    const index = match.index || 0;
    out += escapeHtml(text.slice(last, index));
    let cls: string;
    if (language === 'css') {
      if (part.startsWith('/*')) cls = 'tok-comment';
      else if (part.startsWith('#') || (part.startsWith('"') && part.includes(':')) || part.startsWith("'")) cls = 'tok-string';
      else if (/^[\w-]+(?=:)/.test(part)) cls = 'tok-keyword';
      else if (/[:;{},]/.test(part)) cls = 'tok-keyword';
      else if (/^\d/.test(part)) cls = 'tok-number';
      else cls = 'tok-string';
    } else if (language === 'html' || language === 'xml') {
      if (part.startsWith('<!--')) cls = 'tok-comment';
      else if (part.startsWith('<')) cls = 'tok-keyword';
      else cls = 'tok-string';
    } else if (language === 'json') {
      const m = part.match(/"([^"]*)":/);
      cls = 'tok-keyword';
    } else if (language === 'markdown') {
      if (part.startsWith('#')) cls = 'tok-keyword';
      else if (part.startsWith('`')) cls = 'tok-string';
      else if (part.startsWith('[')) cls = 'tok-string';
      else cls = 'tok-string';
    } else {
      // Standard: comment, string, number, keyword
      if (part.startsWith('//') || part.startsWith('/*') || part.startsWith('#') || part.startsWith('--') || part.startsWith('<!--')) cls = 'tok-comment';
      else if (part.startsWith('"') || part.startsWith("'") || part.startsWith('`') || part.startsWith('r"') || part.startsWith('r#"') || part.startsWith('b\'')) cls = 'tok-string';
      else if (/^\d/.test(part)) cls = 'tok-number';
      else cls = 'tok-keyword';
    }
    out += `<span class="${cls}">${escapeHtml(part)}</span>`;
    last = index + part.length;
  }
  return out + escapeHtml(text.slice(last));
}
function normalizeContent(value: unknown) {
  return normalizeMessageParts(value).content;
}
function rawToolName(raw: any) {
  const candidates = [raw.toolName, raw.tool_name, raw.name, raw.tool, raw.recipient_name, raw.function, raw.source];
  for (const value of candidates) {
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  const content = asRecordish(raw.content);
  for (const key of ['source', 'tool_name', 'name', 'tool', 'recipient_name', 'function']) {
    const value = content?.[key];
    if (typeof value === 'string' && value.trim()) return value.trim();
  }
  return undefined;
}
function rawToolInput(raw: any) {
  if (raw?.role !== 'tool') return undefined;
  const candidates = [raw.arguments, raw.args, raw.params, raw.parameters, raw.input, raw.tool_input, raw.tool_args, raw.request, raw.tool_call?.arguments, raw.tool_call?.args, raw.tool_call?.params, raw.tool_call?.parameters];
  const fn = asRecordish(raw.function);
  if (fn) candidates.push(fn.arguments, fn.args, fn.params, fn.parameters);
  for (const value of candidates) {
    if (value !== undefined && value !== null && value !== '') return value;
  }
  return undefined;
}
function asRecordish(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}
function normalizeMessage(raw: any): ChatMessage {
  const parts = normalizeMessageParts(raw.content, raw);
  return {
    id: String(raw.id || uid('m')),
    role: ['user', 'assistant', 'tool', 'system'].includes(raw.role) ? raw.role : 'system',
    content: parts.content,
    reasoning: parts.reasoning,
    timestamp: raw.timestamp,
    toolName: rawToolName(raw),
    toolInput: rawToolInput(raw),
  };
}
function mergeWatchedMessage(prev: ChatMessage[], msg: ChatMessage): ChatMessage[] {
  if (prev.some((m) => m.id === msg.id)) return prev.map((m) => m.id === msg.id ? { ...m, ...msg } : m);
  // Match by content+role for user messages (server ID differs from local uid)
  if (msg.role === 'user') {
    const existing = prev.findIndex((m) => m.role === 'user' && m.content === msg.content);
    if (existing >= 0) return prev.map((m, i) => i === existing ? { ...m, ...msg } : m);
  }
  // Match pending assistant placeholder for assistant messages from watch.
  // Local browser-originated streams use a temporary assistant_* id; once the
  // server persists the same assistant message, the watch endpoint sends the
  // real id. Merge that persisted copy back into the local card instead of
  // appending a duplicate after the first streamed turn completes.
  if (msg.role === 'assistant') {
    const pendingIdx = prev.findIndex((m) => m.pending && (m.id === OTHER_PLATFORM_PENDING_ID || m.id.startsWith('assistant_')));
    if (pendingIdx >= 0) return prev.map((m, i) => i === pendingIdx ? { ...m, ...msg, pending: false } : m);
    const localStreamIdx = prev.findIndex((m) => m.role === 'assistant' && m.id.startsWith('assistant_') && m.content === msg.content && (m.reasoning || '') === (msg.reasoning || ''));
    if (localStreamIdx >= 0) return prev.map((m, i) => i === localStreamIdx ? { ...m, ...msg, pending: false } : m);
  }
  const withoutStalePending = msg.role === 'assistant'
    ? prev.filter((m) => !(m.pending && m.id === OTHER_PLATFORM_PENDING_ID))
    : prev;
  const next = [...withoutStalePending, msg];
  if (msg.role === 'user' && !next.some((m) => m.pending && m.id === OTHER_PLATFORM_PENDING_ID)) {
    next.push({ id: OTHER_PLATFORM_PENDING_ID, role: 'assistant', content: '', pending: true });
  }
  return next.slice(-MESSAGE_WINDOW);
}
function isNearBottom(el: HTMLElement | null, px = 120) {
  if (!el) return true;
  return el.scrollHeight - el.scrollTop - el.clientHeight < px;
}
function readPinnedIds() {
  try { return new Set<string>(JSON.parse(localStorage.getItem('pinnedSessions') || '[]')); }
  catch { return new Set<string>(); }
}
function realModelOrEmpty(value: unknown) {
  const modelId = String(value || '').trim();
  return modelId && modelId !== 'hermes-agent' ? modelId : '';
}
function readStoredModel() {
  const stored = localStorage.getItem('model') || '';
  if (stored === 'hermes-agent') {
    localStorage.removeItem('model');
    return '';
  }
  return realModelOrEmpty(stored);
}

function useLongPressContextMenu(openAt: (x: number, y: number) => void) {
  const timer = useRef<number | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const clear = useCallback(() => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    start.current = null;
  }, []);
  const onPointerDown = useCallback((event: React.PointerEvent) => {
    if (event.pointerType === 'mouse') return;
    if ((event.target as HTMLElement).closest('button,a,input,textarea,select')) return;
    const x = event.clientX;
    const y = event.clientY;
    start.current = { x, y };
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* ignore */ }
    timer.current = window.setTimeout(() => {
      timer.current = null;
      start.current = null;
      openAt(x, y);
    }, 520);
  }, [openAt]);
  const onPointerMove = useCallback((event: React.PointerEvent) => {
    const point = start.current;
    if (!point) return;
    if (Math.hypot(event.clientX - point.x, event.clientY - point.y) > 12) clear();
  }, [clear]);
  return { onPointerDown, onPointerMove, onPointerUp: clear, onPointerCancel: clear };
}

function flattenModelOptions(body: any): ModelOption[] {
  const seen = new Set<string>();
  const out: ModelOption[] = [];
  const push = (id: unknown, label?: unknown, provider?: unknown) => {
    const modelId = String(id || '').trim();
    if (!modelId || modelId === 'hermes-agent' || seen.has(modelId)) return;
    seen.add(modelId);
    const providerName = String(provider || '').trim();
    out.push({ id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined });
  };
  if (Array.isArray(body?.providers)) {
    for (const provider of body.providers) {
      const providerId = provider?.slug || provider?.provider || provider?.id || provider?.name || '';
      const providerLabel = provider?.name || providerId;
      for (const modelId of provider?.models || []) push(modelId, `${providerLabel} · ${modelId}`, providerId);
    }
  }
  for (const modelRow of body?.data || []) push(modelRow?.id || modelRow, modelRow?.label || modelRow?.id, modelRow?.provider);
  return out;
}

export default function App() {
  const [mode, setMode] = useState<Mode>(initialRoute.mode || 'chat');
  const [lang, setLangState] = useState<Lang>(initLang);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(initialRoute.mode === 'images' || initialRoute.mode === 'memory' || initialRoute.mode === 'insights' || initialRoute.mode === 'settings');
  const [workspaceCollapsed, setWorkspaceCollapsed] = useState(true);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [theme, setTheme] = useState<Theme>(() => normalizeTheme(localStorage.getItem('theme')));
  const [apiBase, setApiBase] = useState(() => localStorage.getItem('apiBase') || DEFAULT_API_BASE);
  const [apiKey, setApiKey] = useState(() => localStorage.getItem('apiKey') || '');
  const [models, setModels] = useState<ModelOption[]>([]);
  const [model, setModelState] = useState(readStoredModel);
  const [selectedModelProvider, setSelectedModelProvider] = useState('');
  const [followUpBehaviour, setFollowUpBehaviour] = useState<FollowUpBehaviour>(() => normalizeFollowUpBehaviour(localStorage.getItem(FOLLOW_UP_BEHAVIOUR_KEY)));
  const [composerEnterMode, setComposerEnterMode] = useState<ComposerEnterMode>(() => normalizeComposerEnterMode(localStorage.getItem(COMPOSER_ENTER_MODE_KEY)));
  const [followUpQueues, setFollowUpQueues] = useState<Record<string, FollowUpQueueItem[]>>(readFollowUpQueues);
  const [effort, setEffort] = useState<(typeof EFFORTS)[number]>(() => (localStorage.getItem('effort') as (typeof EFFORTS)[number]) || 'medium');
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string>(initialRoute.mode === 'chat' ? initialRoute.sessionId || '' : '');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [showReasoning, setShowReasoning] = useState(() => localStorage.getItem('showReasoning') === '1');
  const [hasOlder, setHasOlder] = useState(false);
  const [hasNewer, setHasNewer] = useState(false);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [input, setInput] = useState('');
  const [composerCompact, setComposerCompact] = useState(false);
  const [filter, setFilter] = useState('');
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const setStatus = useCallback((_value: string) => {}, []);
  const [busy, setBusy] = useState(false);
  const [newMessageCount, setNewMessageCount] = useState(0);
  const [workspacePath, setWorkspacePath] = useState('');
  const [workspaceEntries, setWorkspaceEntries] = useState<WorkspaceEntry[]>([]);
  const [skillList, setSkillList] = useState<Skill[]>([]);
  const [selectedSkillName, setSelectedSkillName] = useState('');
  const [skillFileTree, setSkillFileTree] = useState<Record<string, WorkspaceEntry[]>>({});
  const [expandedSkillPaths, setExpandedSkillPaths] = useState<Set<string>>(() => new Set(['']));
  const [skillPreview, setSkillPreview] = useState<WorkspacePreview>({ path: '', content: '', kind: 'none' });
  const [workspaceTree, setWorkspaceTree] = useState<Record<string, WorkspaceEntry[]>>({});
  const [expandedWorkspacePaths, setExpandedWorkspacePaths] = useState<Set<string>>(() => new Set(['']));
  const [preview, setPreview] = useState<WorkspacePreview>({ path: '', content: '', kind: 'none' });
  const [dialog, setDialog] = useState<DialogState>(null);
  const [activeSessionDetail, setActiveSessionDetail] = useState<Session | null>(null);
  const [pinnedIds, setPinnedIds] = useState<Set<string>>(readPinnedIds);
  const [sessionMenu, setSessionMenu] = useState<SessionContextMenu>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<WorkspaceContextMenu>(null);
  const [cronJobs, setCronJobs] = useState<Job[]>([]);
  const [cronName, setCronName] = useState('');
  const [cronSchedule, setCronSchedule] = useState('0 9 * * *');
  const [cronPrompt, setCronPrompt] = useState('');
  const [cronScript, setCronScript] = useState('');
  const [cronDeliver, setCronDeliver] = useState('');
  const [cronEditingId, setCronEditingId] = useState(initialRoute.mode === 'cron' ? initialRoute.jobId || '' : '');
  const [skillFilter, setSkillFilter] = useState('');
  const [skillRouteTarget, setSkillRouteTarget] = useState(initialRoute.mode === 'skills' ? initialRoute.skillName || '' : '');
  const [expandedSkillCats, setExpandedSkillCats] = useState<Set<string>>(new Set());
  const [workspaceRouteTarget, setWorkspaceRouteTarget] = useState<{ workspaceKind: 'file' | 'folder'; workspacePath: string; workspaceEdit?: boolean } | null>(initialRoute.mode === 'workspace' && initialRoute.workspaceKind ? { workspaceKind: initialRoute.workspaceKind, workspacePath: initialRoute.workspacePath || '' } : null);
  const [usageInsights, setUsageInsights] = useState<UsageInsights | null>(null);
  const [usageLoading, setUsageLoading] = useState(false);
  const [usageError, setUsageError] = useState('');
  const [usagePeriod, setUsagePeriod] = useState<1 | 7 | 30>(7);
  const [usageMetric, setUsageMetric] = useState<UsageMetric>('total_tokens');
  const [initialImageFilename, setInitialImageFilename] = useState(initialRoute.mode === 'images' ? initialRoute.imageFilename || '' : '');
  const chatScrollRef = useRef<HTMLElement | null>(null);
  const composerRef = useRef<HTMLElement | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const messageRequestRef = useRef(0);
  const searchVersionRef = useRef(0);
  const modelRef = useRef(model);
  const providerRef = useRef(selectedModelProvider);
  useEffect(() => { modelRef.current = model; }, [model]);
  useEffect(() => { providerRef.current = selectedModelProvider; }, [selectedModelProvider]);
  const scrollLatestAfterRenderRef = useRef(false);
  const titleRefreshDoneRef = useRef<Set<string>>(new Set());
  const skipNextHistoryLoadRef = useRef('');
  const watchSourceRef = useRef<EventSource | null>(null);

  const headers = useCallback((json = true) => {
    const h: Record<string, string> = {};
    if (json) h['Content-Type'] = 'application/json';
    if (apiKey && !apiBase.startsWith('/')) h.Authorization = `Bearer ${apiKey}`;
    return h;
  }, [apiBase, apiKey]);

  const requestPrompt = useCallback((title: string, message: string, value = '') => new Promise<string | null>((resolve) => {
    setDialog({ variant: 'prompt', title, message, value, resolve });
  }), []);
  const requestConfirm = useCallback((title: string, message: string, danger = false) => new Promise<boolean>((resolve) => {
    setDialog({ variant: 'confirm', title, message, danger, resolve });
  }), []);

  const writeHashRoute = useCallback((route: HashRoute) => {
    const nextHash = buildHashRoute(route);
    if (window.location.hash !== nextHash) window.history.replaceState(null, '', nextHash);
  }, []);
  const clearSelectedSkill = useCallback(() => {
    setSelectedSkillName('');
    setSkillRouteTarget('');
    setExpandedSkillPaths(new Set(['']));
    setSkillFileTree({});
    setSkillPreview({ path: '', content: '', kind: 'none' });
  }, []);
  const applyHashRoute = useCallback((route: HashRoute) => {
    setMode(route.mode);
    setSidebarCollapsed(route.mode === 'images' || route.mode === 'memory' || route.mode === 'insights' || route.mode === 'settings');
    if (route.mode !== 'chat' && route.mode !== 'cron') setMobileSidebarOpen(false);
    if (route.mode === 'chat' && route.sessionId) setActiveSessionId(route.sessionId);
    if (route.mode === 'cron' && route.jobId) setCronEditingId(route.jobId);
    if (route.mode === 'skills' && route.skillName) setSkillRouteTarget(route.skillName);
    if (route.mode === 'skills' && !route.skillName) clearSelectedSkill();
    if (route.mode === 'images') setInitialImageFilename(route.imageFilename || '');
    if (route.mode === 'workspace' && route.workspaceKind) setWorkspaceRouteTarget({ workspaceKind: route.workspaceKind, workspacePath: route.workspacePath || '' });
  }, [clearSelectedSkill]);
  useEffect(() => {
    const applyCurrentHashRoute = () => applyHashRoute(getCurrentHashRoute());
    window.addEventListener('hashchange', applyCurrentHashRoute);
    applyCurrentHashRoute();
    return () => window.removeEventListener('hashchange', applyCurrentHashRoute);
  }, [applyHashRoute]);

  useEffect(() => { document.documentElement.classList.toggle('dark', isDarkTheme(theme)); document.documentElement.dataset.theme = theme; delete document.documentElement.dataset.skin; localStorage.setItem('theme', theme); localStorage.removeItem('skin'); }, [theme]);
  useEffect(() => localStorage.setItem('apiBase', apiBase), [apiBase]);
  useEffect(() => localStorage.setItem('apiKey', apiKey), [apiKey]);
  useEffect(() => { const next = realModelOrEmpty(model); if (next) localStorage.setItem('model', next); }, [model]);
  useEffect(() => localStorage.setItem(FOLLOW_UP_BEHAVIOUR_KEY, followUpBehaviour), [followUpBehaviour]);
  useEffect(() => localStorage.setItem(COMPOSER_ENTER_MODE_KEY, composerEnterMode), [composerEnterMode]);
  useEffect(() => localStorage.setItem('effort', effort), [effort]);
  useEffect(() => localStorage.setItem('showReasoning', showReasoning ? '1' : '0'), [showReasoning]);
  useEffect(() => localStorage.setItem('pinnedSessions', JSON.stringify(Array.from(pinnedIds))), [pinnedIds]);

  const activeSession = sessions.find((s) => s.id === activeSessionId) || (activeSessionDetail?.id === activeSessionId ? activeSessionDetail : undefined);
  useEffect(() => {
    const activeModel = realModelOrEmpty(activeSession?.model);
    const activeProvider = String(activeSession?.provider || '').trim();
    if (activeModel) {
      modelRef.current = activeModel;
      providerRef.current = activeProvider;
      setModelState((current) => activeModel !== current ? activeModel : current);
      setSelectedModelProvider((current) => activeProvider !== current ? activeProvider : current);
    }
  }, [activeSession?.model, activeSession?.provider]);

  const filteredSessions = useMemo(() => {
    const pinned = sessions.filter((s) => pinnedIds.has(s.id));
    const normal = sessions.filter((s) => !pinnedIds.has(s.id));
    return { pinned, normal };
  }, [sessions, pinnedIds]);

  const loadModels = useCallback(async () => {
    try {
      const res = await fetch('/models-cache');
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      const list = flattenModelOptions(body);
      const current = realModelOrEmpty(activeSession?.model) || realModelOrEmpty(model);
      if (list.length) {
        const provider = current ? list.find((item) => item.id === current)?.provider || '' : '';
        setModels(list);
        if (!current) setModelState(list[0].id);
        setSelectedModelProvider(provider);
      }
      setStatus(t('status.modelsLoaded'));
    } catch (err: any) { setStatus(`Models unavailable: ${err.message}`); }
  }, [activeSession?.model, model, setStatus]);

  const loadUsageInsights = useCallback(async () => {
    setUsageLoading(true);
    setUsageError('');
    try {
      const usageRes = await fetch('/insights/usage');
      if (!usageRes.ok) throw new Error(await usageRes.text());
      setUsageInsights(await usageRes.json());
    } catch (err: any) {
      setUsageError(err?.message || 'Usage insights unavailable');
    } finally {
      setUsageLoading(false);
    }
  }, []);

  const loadSessions = useCallback(async (query = filter) => {
    const version = ++searchVersionRef.current;
    try {
      const params = new URLSearchParams({ limit: '80' });
      if (query.trim()) params.set('q', query.trim());
      const res = await fetch(`/sessions/search?${params}`, { headers: headers(false) });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      if (version !== searchVersionRef.current) return;
      const list: Session[] = body.data || [];
      setSessions(list);
      if (!activeSessionId && list.length) setActiveSessionId(list[0].id);
      setStatus(t('chat.connected'));
    } catch (err: any) { setStatus(`Sessions unavailable: ${err.message}`); }
  }, [activeSessionId, filter, headers]);

  const loadSessionDetail = useCallback(async (sessionId: string) => {
    if (!sessionId) return;
    if (sessionId === DRAFT_SESSION_ID) return;
    try {
      const res = await fetch(apiJoin(apiBase, `/api/sessions/${encodeURIComponent(sessionId)}`), { headers: headers(false) });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      const detail = (body.data || body.session || body) as Session;
      setActiveSessionDetail(detail);
      setSessions((old) => old.some((s) => s.id === detail.id) ? old.map((s) => s.id === detail.id ? { ...s, ...detail } : s) : [detail, ...old]);
    } catch (err: any) { setStatus(`Session detail unavailable: ${err.message}`); }
  }, [apiBase, headers]);


  const changeSessionModel = useCallback((nextModel: string, option?: ModelOption) => {
    const resolvedModel = realModelOrEmpty(nextModel);
    if (!resolvedModel) return;
    const provider = String(option?.provider || '').trim();
    modelRef.current = resolvedModel;
    providerRef.current = provider;
    setModelState(resolvedModel);
    setSelectedModelProvider(provider);
    if (activeSessionId === DRAFT_SESSION_ID) setActiveSessionDetail((old) => old ? { ...old, model: resolvedModel, provider } : old);
    if (activeSessionId && activeSessionId !== DRAFT_SESSION_ID) {
      setActiveSessionDetail((old) => old?.id === activeSessionId ? { ...old, model: resolvedModel, provider } : old);
      setSessions((old) => old.map((s) => s.id === activeSessionId ? { ...s, model: resolvedModel, provider } : s));
    }
    setStatus('Session model selected');
  }, [activeSessionId]);

  const refreshSessionTitleOnce = useCallback(async (sessionId: string) => {
    if (!sessionId || sessionId === DRAFT_SESSION_ID) return;
    if (titleRefreshDoneRef.current.has(sessionId)) return;
    titleRefreshDoneRef.current.add(sessionId);
    await new Promise((resolve) => window.setTimeout(resolve, 450));
    await loadSessionDetail(sessionId);
  }, [loadSessionDetail]);

  const createSession = useCallback(async () => {
    const sessionModel = realModelOrEmpty(modelRef.current) || models[0]?.id || '';
    const sessionProvider = providerRef.current;
    const sessionBody = sessionProvider ? { model: sessionModel, provider: sessionProvider } : { model: sessionModel };
    const res = await fetch(apiJoin(apiBase, '/api/sessions'), { method: 'POST', headers: headers(), body: JSON.stringify(sessionBody) });
    if (!res.ok) throw new Error(await res.text());
    const body = await res.json();
    return (body.session || body.data || body) as Session;
  }, [apiBase, headers, models]);

  const startDraftSession = useCallback(() => {
    const sessionModel = realModelOrEmpty(model) || models[0]?.id || '';
    messageRequestRef.current += 1;
    setActiveSessionId(DRAFT_SESSION_ID);
    setActiveSessionDetail({ id: DRAFT_SESSION_ID, model: sessionModel, provider: selectedModelProvider });
    setMessages([]);
    setHasOlder(false);
    setHasNewer(false);
    setInput('');
    setAttachments([]);
    setStatus('Draft conversation');
    setSessionMenu(null);
    writeHashRoute({ mode: 'chat' });
  }, [model, models, selectedModelProvider, writeHashRoute]);

  const loadMessageWindow = useCallback(async (sessionId: string, direction: 'latest' | 'older' | 'newer' = 'latest') => {
    if (sessionId === DRAFT_SESSION_ID) return;
    if (!sessionId) return;
    if (loadingMessages && direction !== 'latest') return;
    const scroller = chatScrollRef.current;
    const oldHeight = scroller?.scrollHeight || 0;
    const req = ++messageRequestRef.current;
    setLoadingMessages(true);
    try {
      const params = new URLSearchParams({ limit: String(MESSAGE_PAGE) });
      if (direction === 'older') {
        const before = numericId(messages[0]?.id);
        if (!before) return;
        params.set('before', before);
      }
      if (direction === 'newer') {
        const after = numericId(messages[messages.length - 1]?.id);
        if (!after) return;
        params.set('after', after);
      }
      const res = await fetch(`/chat/messages/${encodeURIComponent(sessionId)}?${params}`);
      if (!res.ok) throw new Error(await res.text());
      const page: MessagePage = await res.json();
      if (req !== messageRequestRef.current) return;
      const chunk = (page.data || []).filter((m: any) => ['user', 'assistant', 'tool', 'system'].includes(m.role)).map(normalizeMessage);
      if (direction === 'older') {
        setMessages((old) => [...chunk, ...old].slice(0, MESSAGE_WINDOW));
        setHasOlder(page.has_older);
        setHasNewer(true);
        requestAnimationFrame(() => {
          if (scroller) scroller.scrollTop += scroller.scrollHeight - oldHeight;
        });
      } else if (direction === 'newer') {
        setMessages((old) => [...old, ...chunk].slice(-MESSAGE_WINDOW));
        setHasNewer(page.has_newer);
        setHasOlder(true);
      } else {
        scrollLatestAfterRenderRef.current = true;
        setMessages(chunk);
        setHasOlder(page.has_older);
        setHasNewer(page.has_newer);
      }
    } catch (err: any) { setStatus(`Messages unavailable: ${err.message}`); }
    finally { setLoadingMessages(false); }
  }, [loadingMessages, messages]);

  const fetchWorkspaceEntries = useCallback(async (path = '') => {
    const res = await fetch(`/workspace/list?path=${encodeURIComponent(path || '')}`);
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    const body = await res.json();
    return { path: body.path || path || '', entries: (body.entries || []) as WorkspaceEntry[] };
  }, []);

  const loadWorkspace = useCallback(async (path = workspacePath) => {
    try {
      const body = await fetchWorkspaceEntries(path || '');
      setWorkspacePath(body.path);
      setWorkspaceEntries(body.entries);
      setWorkspaceTree((old) => ({ ...old, [body.path]: body.entries }));
    } catch (err: any) { setWorkspaceEntries([]); setStatus(`Workspace unavailable: ${err.message}`); }
  }, [fetchWorkspaceEntries, workspacePath]);

  const toggleWorkspaceFolder = useCallback(async (entry: WorkspaceEntry) => {
    if (entry.kind !== 'dir') return;
    const path = entry.path || '';
    if (expandedWorkspacePaths.has(path)) {
      setExpandedWorkspacePaths((old) => { const next = new Set(old); next.delete(path); return next; });
      return;
    }
    try {
      if (!workspaceTree[path]) {
        const body = await fetchWorkspaceEntries(path);
        setWorkspaceTree((old) => ({ ...old, [body.path]: body.entries }));
      }
      setExpandedWorkspacePaths((old) => new Set(old).add(path));
    } catch (err: any) { setStatus(`Workspace folder unavailable: ${err.message}`); }
  }, [expandedWorkspacePaths, fetchWorkspaceEntries, workspaceTree]);

  const selectedSkill = skillList.find((skill) => skill.name === selectedSkillName) || null;
  const loadSkillFiles = useCallback(async (skill: Skill, path = '') => {
    const res = await fetch(`/skills/files?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(path)}`);
    if (!res.ok) throw new Error(await res.text());
    const body = await res.json();
    setSkillFileTree((old) => ({ ...old, [body.path || '']: (body.entries || []) as WorkspaceEntry[] }));
    return body;
  }, []);
  const openSkillFile = useCallback(async (skillName: string, path = 'SKILL.md') => {
    const res = await fetch(`/skills/file?name=${encodeURIComponent(skillName)}&path=${encodeURIComponent(path)}`);
    if (!res.ok) { setStatus(`Skill file unavailable: ${await res.text()}`); return; }
    const blob = await res.blob();
    if (blob.type.startsWith('image/')) setSkillPreview({ path, content: '', kind: 'image', url: URL.createObjectURL(blob) });
    else setSkillPreview({ path, content: await blob.text(), kind: 'text' });
  }, []);
  const selectSkill = useCallback(async (skill: Skill, options?: { writeRoute?: boolean }) => {
    setSelectedSkillName(skill.name);
    setSkillRouteTarget('');
    setExpandedSkillPaths(new Set(['']));
    setSkillFileTree({});
    if (options?.writeRoute !== false) writeHashRoute({ mode: 'skills', skillName: skill.name });
    try {
      await loadSkillFiles(skill, '');
      await openSkillFile(skill.name, 'SKILL.md');
    } catch (err: any) { setStatus(`Skill unavailable: ${err.message}`); }
  }, [loadSkillFiles, openSkillFile, writeHashRoute]);
  const loadSkills = useCallback(async () => {
    try {
      const res = await fetch('/skills/list', { cache: 'no-store' });
      if (!res.ok) throw new Error(await res.text());
      const body = await res.json();
      const list: Skill[] = body.data || body.skills || [];
      setSkillList(list);
    } catch (err: any) { setStatus(`Skills: ${err.message}`); }
  }, []);
  const toggleSkillEnabled = useCallback(async (skill: Skill, enabled: boolean) => {
    const res = await fetch(`/skills/toggle/${encodeURIComponent(skill.name)}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ enabled }) });
    if (!res.ok) { setStatus(`Skill toggle failed: ${await res.text()}`); return; }
    setSkillList((old) => old.map((item) => item.name === skill.name ? { ...item, enabled } : item));
    setStatus(`${enabled ? 'Enabled' : 'Disabled'} skill: ${skill.name}`);
  }, []);
  const toggleSkillFolder = useCallback(async (entry: WorkspaceEntry) => {
    if (!selectedSkill || entry.kind !== 'dir') return;
    const path = entry.path || '';
    if (expandedSkillPaths.has(path)) {
      setExpandedSkillPaths((old) => { const next = new Set(old); next.delete(path); return next; });
      return;
    }
    try {
      await loadSkillFiles(selectedSkill, path);
      setExpandedSkillPaths((old) => new Set(old).add(path));
    } catch (err: any) { setStatus(`Skill folder unavailable: ${err.message}`); }
  }, [expandedSkillPaths, loadSkillFiles, selectedSkill]);

  const resetCronForm = useCallback(() => { setCronName(''); setCronSchedule('0 9 * * *'); setCronPrompt(''); setCronScript(''); setCronDeliver(''); setCronEditingId(''); }, []);
  const loadCronJobs = useCallback(async () => {
    try {
      const res = await fetch(apiJoin(apiBase, '/api/jobs'), { headers: headers(false) });
      if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
      const body = await res.json();
      const nextJobs = body.data || body.jobs || [];
      setCronJobs(nextJobs);
      if (cronEditingId && !nextJobs.some((job: Job) => jobId(job) === cronEditingId)) resetCronForm();
    } catch (err: any) { setStatus(`Jobs unavailable: ${err.message}`); }
  }, [apiBase, cronEditingId, headers, resetCronForm]);
  const beginCronEdit = useCallback((job: Job) => {
    const values = cronEditableValues(job);
    setCronEditingId(jobId(job));
    setCronName(values.name);
    setCronSchedule(values.schedule || '0 9 * * *');
    setCronPrompt(values.prompt);
    setCronScript(values.script);
    setCronDeliver(job.deliver || '');
    writeHashRoute({ mode: 'cron', jobId: jobId(job) });
    buildHashRoute({ mode: 'cron', jobId: jobId(job) });
  }, [writeHashRoute]);
  const saveCronJob = useCallback(async () => {
    if (!cronEditingId) {
      const body = cronScript ? { name: cronName, schedule: cronSchedule, prompt: cronPrompt, script: cronScript } : { name: cronName, schedule: cronSchedule, prompt: cronPrompt };
      const res = await fetch(apiJoin(apiBase, '/api/jobs'), { method: 'POST', headers: headers(), body: JSON.stringify(body) });
      if (!res.ok) { setStatus(await res.text()); return; }
      const bodyJson = await res.json().catch(() => ({}));
      const created = (bodyJson.data || bodyJson.job || bodyJson) as Job;
      const id = jobId(created);
      if (id) setCronEditingId(id);
      setStatus('Cron job saved');
      await loadCronJobs();
      return;
    }
    const patchBody = buildCronPatch({ name: cronName, schedule: cronSchedule, prompt: cronPrompt, script: cronScript });
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}`), { method: 'PATCH', headers: headers(), body: JSON.stringify(patchBody) });
    if (!res.ok) { setStatus(await res.text()); return; }
    await loadCronJobs();
    setStatus('Cron job saved');
  }, [apiBase, cronEditingId, cronName, cronPrompt, cronSchedule, cronScript, headers, loadCronJobs]);
  const runCronJob = useCallback(async () => {
    if (!cronEditingId) return;
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}/run`), { method: 'POST', headers: headers(false) });
    if (!res.ok) { setStatus(await res.text()); return; }
    await loadCronJobs();
    setStatus('Cron job started');
  }, [apiBase, cronEditingId, headers, loadCronJobs]);
  const deleteCronJob = useCallback(async () => {
    if (!cronEditingId) return;
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}`), { method: 'DELETE', headers: headers(false) });
    if (!res.ok) { setStatus(await res.text()); return; }
    resetCronForm();
    await loadCronJobs();
    setStatus('Cron job deleted');
  }, [apiBase, cronEditingId, headers, loadCronJobs, resetCronForm]);

  useEffect(() => { loadModels(); loadWorkspace(''); }, []);
  useEffect(() => { if (mode === 'insights' && !usageInsights && !usageLoading) loadUsageInsights(); }, [mode, usageInsights, usageLoading, loadUsageInsights]);
  useEffect(() => { const t = window.setTimeout(() => loadSessions(filter), 180); return () => window.clearTimeout(t); }, [filter, loadSessions]);
  useEffect(() => { if (mode === 'cron') loadCronJobs(); }, [mode, loadCronJobs]);
  useEffect(() => { if (mode === 'skills') loadSkills(); }, [mode, loadSkills]);
  useEffect(() => {
    if (mode !== 'skills' || !skillRouteTarget || !skillList.length) return;
    const skill = skillList.find((item) => item.name === skillRouteTarget);
    if (skill) selectSkill(skill, { writeRoute: false });
  }, [mode, selectSkill, skillList, skillRouteTarget]);
  useEffect(() => {
    const route = getCurrentHashRoute();
    if (route.mode === 'cron' && route.jobId) {
      const selectedJob = cronJobs.find((job) => jobId(job) === route.jobId);
      if (selectedJob) beginCronEdit(selectedJob);
      else setCronEditingId(route.jobId);
    }
  }, [cronJobs, beginCronEdit]);
  useEffect(() => { if (activeSessionId) { loadSessionDetail(activeSessionId); if (skipNextHistoryLoadRef.current === activeSessionId) { skipNextHistoryLoadRef.current = ''; return; } loadMessageWindow(activeSessionId, 'latest'); } }, [activeSessionId]);
  useEffect(() => {
    if (watchSourceRef.current) { watchSourceRef.current.close(); watchSourceRef.current = null; }
    setNewMessageCount(0);
    if (!activeSessionId || activeSessionId === DRAFT_SESSION_ID) return;
    const es = new EventSource(`/chat/watch/${encodeURIComponent(activeSessionId)}`);
    watchSourceRef.current = es;
    const scrollToBottom = () => { if (chatScrollRef.current && isNearBottom(chatScrollRef.current)) { chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; } };
    es.onmessage = (ev) => {
      try {
        const raw = JSON.parse(ev.data);
        const msg = normalizeMessage(raw);
        let didAppend = false;
        setMessages((prev) => {
          const next = mergeWatchedMessage(prev, msg);
          didAppend = next.length > prev.length || (next.length === prev.length && next[next.length - 1]?.id !== prev[prev.length - 1]?.id);
          return next;
        });
        const wasNearBottom = chatScrollRef.current && isNearBottom(chatScrollRef.current);
        scrollToBottom();
        if (wasNearBottom) {
          setNewMessageCount(0);
        } else if (didAppend) {
          setNewMessageCount((n) => n + 1);
        }
        setStatus(t('chat.streamingOther'));
      } catch { /* ignore */ }
    };
    es.onerror = () => { watchSourceRef.current = null; };
    return () => { es.close(); watchSourceRef.current = null; };
  }, [activeSessionId]);
  useEffect(() => {
    if (!sessionMenu && !workspaceMenu) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') { setSessionMenu(null); setWorkspaceMenu(null); } };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest('.session-context-menu,.workspace-context-menu')) return;
      setSessionMenu(null);
      setWorkspaceMenu(null);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('pointerdown', onPointerDown, true);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pointerdown', onPointerDown, true);
    };
  }, [sessionMenu, workspaceMenu]);
  useLayoutEffect(() => {
    if (!scrollLatestAfterRenderRef.current) return;
    scrollLatestAfterRenderRef.current = false;
    const scroller = chatScrollRef.current;
    if (!scroller) return;
    const scrollBottom = () => { scroller.scrollTop = scroller.scrollHeight; };
    scrollBottom();
    requestAnimationFrame(scrollBottom);
    window.setTimeout(scrollBottom, 60);
  }, [messages, activeSessionId]);

  const onFiles = async (files: FileList | null) => {
    if (!files?.length) return;
    const next = await Promise.all(Array.from(files).map(readFileAttachment));
    setAttachments((old) => [...old, ...next]);
  };
  const uploadAttachments = async (items: Attachment[]) => {
    if (!items.length) return items;
    const res = await fetch('/chat/attachments', {
      method: 'POST',
      headers: headers(),
      body: JSON.stringify({
        files: items.map((att) => ({
          name: att.name,
          mime: att.mime,
          kind: att.kind,
          data_url: att.dataUrl,
        })),
      }),
    });
    if (!res.ok) throw new Error(await res.text());
    const body = await res.json();
    const saved = Array.isArray(body?.files) ? body.files : [];
    return items.map((att, index) => ({ ...att, uploadedPath: saved[index]?.path || att.uploadedPath }));
  };
  const buildPayload = (text: string, items: Attachment[]) => buildChatInputWithAttachments(text, items);
  const followUpQueue = followUpQueues[followUpQueueKey(activeSessionId)] || [];
  const persistFollowUpQueues = (queues: Record<string, FollowUpQueueItem[]>) => {
    const next = Object.fromEntries(Object.entries(queues).filter(([, items]) => items.length > 0));
    localStorage.setItem(FOLLOW_UP_QUEUES_KEY, JSON.stringify(next));
    return next;
  };
  const updateFollowUpQueue = (sessionId: string, updater: (items: FollowUpQueueItem[]) => FollowUpQueueItem[]) => {
    const key = followUpQueueKey(sessionId);
    setFollowUpQueues((old) => {
      const next = { ...old, [key]: updater(old[key] || []) };
      return persistFollowUpQueues(next);
    });
  };
  const enqueueFollowUp = (text: string, sessionId = activeSessionId) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    updateFollowUpQueue(sessionId, (items) => [...items, { id: uid('fu'), text: trimmed, createdAt: Date.now() }]);
    setStatus(`Queued follow-up: ${trimmed.slice(0, 80)}${trimmed.length > 80 ? '…' : ''}`);
  };
  const removeQueuedFollowUp = (sessionId: string, itemId: string) => updateFollowUpQueue(sessionId, (items) => items.filter((item) => item.id !== itemId));
  const moveQueuedItem = (itemId: string, direction: -1 | 1) => updateFollowUpQueue(activeSessionId, (items) => {
    const index = items.findIndex((item) => item.id === itemId);
    const nextIndex = index + direction;
    if (index < 0 || nextIndex < 0 || nextIndex >= items.length) return items;
    const next = [...items];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    return next;
  });
  const reorderQueuedItem = (fromIndex: number, toIndex: number) => updateFollowUpQueue(activeSessionId, (items) => {
    if (fromIndex < 0 || fromIndex >= items.length || toIndex < 0 || toIndex >= items.length || fromIndex === toIndex) return items;
    const next = [...items];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);
    return next;
  });
  const shiftNextFollowUp = (sessionId: string) => {
    const key = followUpQueueKey(sessionId);
    const current = readFollowUpQueues();
    const [nextItem, ...rest] = current[key] || [];
    if (!nextItem) return null;
    const next = persistFollowUpQueues({ ...current, [key]: rest });
    setFollowUpQueues(next);
    return nextItem;
  };
  const steerFollowUp = async (text: string) => {
    const trimmed = text.trim();
    const sessionId = activeSessionId;
    if (!trimmed) return;
    if (!sessionId || sessionId === DRAFT_SESSION_ID) { enqueueFollowUp(trimmed, sessionId); return; }
    try {
      const sessionModel = realModelOrEmpty(modelRef.current) || activeSession?.model || activeSessionDetail?.model || '';
      const sessionProvider = providerRef.current || activeSession?.provider || activeSessionDetail?.provider || '';
      const res = await fetch(apiJoin(apiBase, `/api/sessions/${encodeURIComponent(sessionId)}/chat`), { method: 'POST', headers: headers(), body: JSON.stringify(buildChatRequestBody(`/steer ${text}`, sessionModel, effort, sessionProvider)) });
      if (!res.ok) throw new Error(await res.text());
      setStatus(`Steered: ${trimmed.slice(0, 80)}${trimmed.length > 80 ? '…' : ''}`);
    } catch (err: any) {
      setStatus(`Steer failed: ${err.message || err}`);
      enqueueFollowUp(trimmed, sessionId);
    }
  };

  const runChatTurn = async (turnText: string, turnAttachments: Attachment[], initialSessionId = activeSessionId, clearComposer = true) => {
    const text = turnText.trim();
    if (!text && turnAttachments.length === 0) return;
    setBusy(true); setStatus('Running');
    let sessionId = initialSessionId;
    let createdSession: Session | null = null;
    try {
      if (!sessionId || sessionId === DRAFT_SESSION_ID) {
        createdSession = await createSession();
        sessionId = createdSession.id;
        skipNextHistoryLoadRef.current = sessionId;
        setActiveSessionId(sessionId);
        setActiveSessionDetail(createdSession);
        setSessions((old) => old.some((s) => s.id === sessionId) ? old.map((s) => s.id === sessionId ? { ...s, ...createdSession } : s) : [createdSession!, ...old]);
        writeHashRoute({ mode: 'chat', sessionId });
        setHasOlder(false);
        setHasNewer(false);
      }
    } catch (err: any) { setStatus(`Cannot create session: ${err.message}`); setBusy(false); return; }
    const stick = isNearBottom(chatScrollRef.current, 180);
    let payloadAttachments: Attachment[] = turnAttachments;
    try {
      payloadAttachments = await uploadAttachments(turnAttachments);
    } catch (err: any) {
      setStatus(`Cannot upload attachments: ${err.message || err}`);
      setBusy(false);
      return;
    }
    const userText = text || payloadAttachments.map((a) => a.name).join(', ');
    const userMsg: ChatMessage = { id: uid('user'), role: 'user', content: userText, timestamp: Date.now() / 1000 };
    const assistantId = uid('assistant');
    const assistantMsg: ChatMessage = { id: assistantId, role: 'assistant', content: '', pending: true };
    const payloadInput = buildPayload(text, payloadAttachments);
    if (createdSession) setMessages(() => [userMsg, assistantMsg]);
    else setMessages((old) => [...old, userMsg, assistantMsg].slice(-MESSAGE_WINDOW));
    setHasNewer(false);
    const sessionModel = realModelOrEmpty(modelRef.current) || createdSession?.model || activeSession?.model || activeSessionDetail?.model || '';
    const sessionProvider = providerRef.current || createdSession?.provider || activeSession?.provider || activeSessionDetail?.provider || '';
    if (clearComposer) { setInput(''); setAttachments([]); }
    setStatus('Running');
    if (stick) requestAnimationFrame(() => { if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; });
    try {
      const res = await fetch(apiJoin(apiBase, `/api/sessions/${encodeURIComponent(sessionId)}/chat/stream`), { method: 'POST', headers: headers(), body: JSON.stringify(buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)) });
      if (!res.ok || !res.body) throw new Error(await res.text());
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';
      let finalText = '';
      let reasoningText = '';
      const scrollWithStream = () => {
        if (isNearBottom(chatScrollRef.current, 220)) requestAnimationFrame(() => { if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; });
      };
      const animator = createStreamAnimator({
        onUpdate: (text) => {
          setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, content: text, pending: true } : m));
          scrollWithStream();
        },
      });
      try {
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });
          const blocks = buffer.split('\n\n');
          buffer = blocks.pop() || '';
          for (const block of blocks) {
            const { event, data } = parseSseBlock(block);
            if (!data) continue;
            let payload: any;
            try { payload = JSON.parse(data); } catch { continue; }
            if (event === 'assistant.delta') {
              const delta = payload.delta || '';
              finalText += delta;
              animator.append(delta);
            }
            if (event === 'reasoning.delta' || event === 'assistant.reasoning.delta' || event === 'thinking.delta' || event === 'assistant.thinking.delta') {
              const delta = payload.delta || payload.text || payload.content || '';
              reasoningText += delta;
              setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, reasoning: reasoningText, pending: true } : m));
            }
            if (event === 'tool.started' || event === 'tool.completed' || event === 'tool.progress') setStatus(event === 'tool.progress' ? (payload.delta || 'thinking') : `${payload.tool_name || 'tool'} ${event.replace('tool.', '')}`);
            if (event === 'assistant.completed') {
              const parts = normalizeMessageParts(payload.content || finalText, payload);
              finalText = parts.content || finalText;
              if (parts.reasoning) reasoningText = parts.reasoning;
              animator.setTarget(finalText);
            }
            if (event === 'run.completed' && payload?.messages?.[0]?.content && !finalText) {
              const messageParts = normalizeMessageParts(payload.messages[0].content, payload.messages[0]);
              finalText = messageParts.content;
              if (messageParts.reasoning) reasoningText = messageParts.reasoning;
              animator.setTarget(finalText);
            }
          }
        }
      } catch (err) {
        animator.cancel();
        throw err;
      }
      // Wait for the client-side typing animation to catch up to the server-provided final text
      // before flipping `pending: false`, so the caret / shimmer / glow run for the full duration.
      await animator.finish(finalText);
      setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, pending: false, content: finalText || m.content, reasoning: reasoningText || m.reasoning } : m));
      setStatus(t('chat.connected'));
      await refreshSessionTitleOnce(sessionId);
      await loadWorkspace(workspacePath);
    } catch (err: any) {
      setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, pending: false, content: `Request failed: ${err.message}` } : m));
      setStatus(`Error: ${err.message}`);
    } finally {
      setBusy(false);
      const nextQueued = shiftNextFollowUp(sessionId);
      if (nextQueued) window.setTimeout(() => runChatTurn(nextQueued.text, [], sessionId, false), 0);
    }
  };

  const sendMessage = async () => {
    const text = input.trim();
    if (!text && attachments.length === 0) return;
    setNewMessageCount(0);
    if (busy) {
      if (!text) return;
      if (followUpBehaviour === 'steer') await steerFollowUp(text);
      else enqueueFollowUp(text);
      setInput('');
      return;
    }
    await runChatTurn(text, attachments);
  };
  const steerQueuedItem = async (item: FollowUpQueueItem) => {
    removeQueuedFollowUp(activeSessionId, item.id);
    await steerFollowUp(item.text);
  };
  const editQueuedItem = (item: FollowUpQueueItem) => {
    removeQueuedFollowUp(activeSessionId, item.id);
    setInput(item.text);
  };

  const downloadEntry = useCallback((entry: WorkspaceEntry) => { const a = document.createElement('a'); a.href = `/workspace/file?path=${encodeURIComponent(entry.path)}&download=1`; a.download = entry.name; a.click(); }, []);
  const openWorkspaceEntry = useCallback(async (entry: WorkspaceEntry, options?: { edit?: boolean; route?: boolean }) => {
    if (entry.kind === 'dir') {
      writeHashRoute({ mode: 'workspace', workspaceKind: 'folder', workspacePath: entry.path });
      await toggleWorkspaceFolder(entry);
      return;
    }
    const res = await fetch(`/workspace/file?path=${encodeURIComponent(entry.path)}`);
    if (!res.ok) { setStatus(`Preview failed: ${res.status}`); return; }
    const blob = await res.blob();
    if (blob.type.startsWith('image/')) {
      setPreview({ path: entry.path, content: '', kind: 'image', url: URL.createObjectURL(blob) });
      if (options?.edit) setStatus('Workspace item is not editable');
    } else if (blob.type.startsWith('text/') || isWorkspaceTextFile(entry.name)) {
      setPreview({ path: entry.path, content: await blob.text(), kind: 'text', editRequest: options?.edit ? Date.now() : undefined });
    } else {
      if (options?.edit) setStatus('Workspace item is not editable');
      else downloadEntry(entry);
    }
    if (options?.route !== false) writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });
    if (options?.route !== false) buildHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });
  }, [downloadEntry, toggleWorkspaceFolder, writeHashRoute]);
  const openWorkspacePathFile = useCallback(async (targetPath: string) => {
    await openWorkspaceEntry({ name: basename(targetPath), path: targetPath, kind: 'file' });
  }, [openWorkspaceEntry]);
  const openWorkspaceRouteTarget = useCallback(async (targetPath: string, workspaceKind: 'file' | 'folder', options?: { edit?: boolean }) => {
    setMode('workspace');
    setSidebarCollapsed(false);
    const parents = workspaceRouteParents(targetPath);
    const foldersToLoad = workspaceKind === 'folder' ? [...parents, targetPath] : parents;
    for (const folderPath of foldersToLoad) {
      const body = await fetchWorkspaceEntries(folderPath);
      setWorkspaceTree((old) => ({ ...old, [body.path]: body.entries }));
      if (folderPath === '') { setWorkspacePath(body.path); setWorkspaceEntries(body.entries); }
    }
    setExpandedWorkspacePaths((old) => new Set([...Array.from(old), ...parents, targetPath]));
    if (workspaceKind === 'file') await openWorkspacePathFile(targetPath);
    if (options?.edit) {
      setPreview((old) => ({ ...old, editRequest: Date.now() }));
    }
  }, [fetchWorkspaceEntries, openWorkspacePathFile]);
  useEffect(() => {
    if (!workspaceRouteTarget) return;
    openWorkspaceRouteTarget(workspaceRouteTarget.workspacePath, workspaceRouteTarget.workspaceKind, { edit: workspaceRouteTarget.workspaceEdit }).catch((err: any) => setStatus(`Workspace route unavailable: ${err.message}`));
    setWorkspaceRouteTarget(null);
  }, [openWorkspaceRouteTarget, workspaceRouteTarget]);
  const parentPath = workspacePath.split('/').filter(Boolean).slice(0, -1).join('/');
  const togglePin = (sessionId: string) => setPinnedIds((old) => { const next = new Set(old); next.has(sessionId) ? next.delete(sessionId) : next.add(sessionId); return next; });
  const openSessionMenuAt = (session: Session, clientX: number, clientY: number) => {
    const x = Math.min(clientX, window.innerWidth - 210);
    const y = Math.min(clientY, window.innerHeight - 112);
    setSessionMenu({ session, x: Math.max(8, x), y: Math.max(8, y) });
  };
  const openSessionMenu = (session: Session, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    openSessionMenuAt(session, event.clientX, event.clientY);
  };
  const renameSession = async (session: Session) => {
    setSessionMenu(null);
    const nextTitle = await requestPrompt(t('chat.renameTitle'), t('chat.renameTitle'), sessionDisplayTitle(session));
    if (nextTitle === null) return;
    const res = await fetch(apiJoin(apiBase, `/api/sessions/${encodeURIComponent(session.id)}`), { method: 'PATCH', headers: headers(), body: JSON.stringify({ title: nextTitle }) });
    if (!res.ok) { setStatus(`Rename failed: ${await res.text()}`); return; }
    const body = await res.json();
    const title = body.title || nextTitle;
    setSessions((old) => old.map((item) => item.id === session.id ? { ...item, title } : item));
    setActiveSessionDetail((old) => old?.id === session.id ? { ...old, title } : old);
    await loadSessions(filter);
    setStatus('Renamed session');
  };
  const deleteSession = async (session: Session) => {
    setSessionMenu(null);
    if (!await requestConfirm(t('chat.deleteTitle'), t('chat.deleteConfirm'), true)) return;
    const res = await fetch(apiJoin(apiBase, `/api/sessions/${encodeURIComponent(session.id)}`), { method: 'DELETE', headers: headers(false) });
    if (!res.ok) { setStatus(`Delete failed: ${await res.text()}`); return; }
    setPinnedIds((old) => { const next = new Set(old); next.delete(session.id); return next; });
    setSessions((old) => {
      const next = old.filter((item) => item.id !== session.id);
      if (activeSessionId === session.id) setActiveSessionId(next[0]?.id || '');
      return next;
    });
    if (activeSessionId === session.id) { setMessages([]); setActiveSessionDetail(null); }
    await loadSessions(filter);
    setStatus('Deleted session');
  };
  const openWorkspaceMenu = (entry: WorkspaceEntry, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.min(event.clientX, window.innerWidth - 190);
    const y = Math.min(event.clientY, window.innerHeight - (entry.kind === 'file' ? 220 : 112));
    setWorkspaceMenu({ entry, x: Math.max(8, x), y: Math.max(8, y) });
  };
  const viewWorkspaceEntry = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    setMode('workspace');
    setSidebarCollapsed(false);
    writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });
    setWorkspaceRouteTarget({ workspaceKind: 'file', workspacePath: entry.path });
  };
  const editWorkspaceEntryPage = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    setMode('workspace');
    setSidebarCollapsed(false);
    writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });
    setWorkspaceRouteTarget({ workspaceKind: 'file', workspacePath: entry.path, workspaceEdit: true });
  };
  const renameWorkspaceEntry = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    const nextName = await requestPrompt('Rename item', 'Choose a new file or folder name.', entry.name);
    if (nextName === null) return;
    const res = await fetch(`/workspace/item?path=${encodeURIComponent(entry.path)}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: nextName }) });
    if (!res.ok) { setStatus(`Workspace rename failed: ${await res.text()}`); return; }
    setPreview((old) => old.path === entry.path ? { path: '', content: '', kind: 'none' } : old);
    await loadWorkspace(workspacePath);
    setStatus('Renamed workspace item');
  };
  const deleteWorkspaceEntry = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    if (!await requestConfirm('Delete workspace item', `Delete workspace ${entry.kind} “${entry.name}”?`, true)) return;
    const res = await fetch(`/workspace/item?path=${encodeURIComponent(entry.path)}`, { method: 'DELETE' });
    if (!res.ok) { setStatus(`Workspace delete failed: ${await res.text()}`); return; }
    setPreview((old) => old.path === entry.path || old.path.startsWith(`${entry.path}/`) ? { path: '', content: '', kind: 'none' } : old);
    await loadWorkspace(workspacePath);
    setStatus('Deleted workspace item');
  };
  const closeMobileSidebar = () => setMobileSidebarOpen(false);
  const toggleMobileSidebar = () => {
    if (!hasMobileDrawer(mode)) return;
    setSidebarCollapsed(false);
    setMobileSidebarOpen((value) => !value);
  };
  const setNavMode = (next: Mode, collapse = false) => {
    setMode(next);
    setSidebarCollapsed(collapse || next === 'memory' || next === 'insights' || next === 'settings');
    setMobileSidebarOpen(false);
    const route: HashRoute = { mode: next } as HashRoute;
    writeHashRoute(route);
  };
  const wideMode = mode !== 'chat';

  return (
    <div className={`app-shell ${wideMode ? 'wide-mode' : ''} ${mode === 'images' ? 'image-mode' : ''} ${mode === 'skills' ? 'skills-mode' : ''} ${sidebarCollapsed ? 'nav-collapsed' : ''} ${mode === 'chat' && workspaceCollapsed ? 'workspace-collapsed' : ''} ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''}`}>
      <aside className={`sidebar ${sidebarCollapsed ? 'collapsed' : ''}`}>
        <div className="rail">
          <button className="rail-btn muted" onClick={() => setSidebarCollapsed((v) => !v)} title={sidebarCollapsed ? t('nav.expandSidebar') : t('nav.collapseSidebar')}>{sidebarCollapsed ? <ChevronRight /> : <ChevronLeft />}</button>
          <button className={`rail-btn nav-chat ${mode === 'chat' ? 'active' : ''}`} onClick={() => setNavMode('chat')} title={t('nav.chat')}><MessageSquare /></button>
          <button className={`rail-btn nav-cron ${mode === 'cron' ? 'active' : ''}`} onClick={() => setNavMode('cron')} title={t('nav.cron')}><CalendarClock /></button>
          <button className={`rail-btn nav-memory ${mode === 'memory' ? 'active' : ''}`} onClick={() => setNavMode('memory')} title={t('nav.memory')}><Brain /></button>
          <button className={`rail-btn nav-insights ${mode === 'insights' ? 'active' : ''}`} onClick={() => setNavMode('insights', true)} title={t('nav.insights')}><LineChart /></button>
          <button className={`rail-btn nav-skills ${mode === 'skills' ? 'active' : ''}`} onClick={() => setNavMode('skills')} title={t('nav.skills')}><Puzzle /></button>
          <button className={`rail-btn nav-images ${mode === 'images' ? 'active' : ''}`} onClick={() => setNavMode('images', true)} title={t('nav.images')}><ImageIcon /></button>
          <button className={`rail-btn nav-workspace ${mode === 'workspace' ? 'active' : ''}`} onClick={() => { setNavMode('workspace'); loadWorkspace(workspacePath); }} title={t('nav.workspace')}><Folder /></button>
          <button className={`rail-btn nav-settings ${mode === 'settings' ? 'active' : ''}`} onClick={() => setNavMode('settings')} title={t('nav.settings')}><Settings /></button>
        </div>
        {!sidebarCollapsed && <div className="left-body">
          {mode === 'chat' ? <ChatSidebar filter={filter} setFilter={setFilter} startDraftSession={startDraftSession} pinnedSessions={filteredSessions.pinned} normalSessions={filteredSessions.normal} activeSessionId={activeSessionId} setActiveSessionId={setActiveSessionId} writeHashRoute={writeHashRoute} closeMobileSidebar={closeMobileSidebar} pinnedIds={pinnedIds} togglePin={togglePin} openSessionMenu={openSessionMenu} openSessionMenuAt={openSessionMenuAt} /> : mode === 'cron' ? <CronSidebar jobs={cronJobs} editingId={cronEditingId} beginCronEdit={beginCronEdit} resetCronForm={resetCronForm} writeHashRoute={writeHashRoute} closeMobileSidebar={closeMobileSidebar} /> : mode === 'workspace' ? <WorkspaceSidebar rootEntries={workspaceTree[''] || workspaceEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openWorkspaceEntry={openWorkspaceEntry} downloadEntry={downloadEntry} openWorkspaceMenu={openWorkspaceMenu} /> : mode === 'skills' ? <SkillsSidebar skills={skillList} activeSkillName={selectedSkillName} selectSkill={selectSkill} toggleSkillEnabled={toggleSkillEnabled} filter={skillFilter} setFilter={setSkillFilter} expandedCats={expandedSkillCats} setExpandedCats={setExpandedSkillCats} closeMobileSidebar={closeMobileSidebar} /> : (mode === 'memory' || mode === 'settings') ? null : <ModeSidebar mode={mode} />}
        </div>}
        {!sidebarCollapsed && <ThemeCard theme={theme} setTheme={setTheme} />}
      </aside>
      {mobileSidebarOpen && <button type="button" className="mobile-sidebar-backdrop" aria-label="Close list" onClick={closeMobileSidebar} />}
      {sessionMenu && <div className="session-context-menu" role="menu" style={{ left: sessionMenu.x, top: sessionMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        <button type="button" role="menuitem" onClick={() => renameSession(sessionMenu.session)}><Pencil /> {t('chat.rename')}</button>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteSession(sessionMenu.session)}><Trash2 /> {t('chat.delete')}</button>
      </div>}
      {workspaceMenu && <div className="workspace-context-menu" role="menu" style={{ left: workspaceMenu.x, top: workspaceMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        {workspaceMenu.entry.kind === 'file' && <><button type="button" role="menuitem" onClick={() => viewWorkspaceEntry(workspaceMenu.entry)}><Eye /> {t('workspace.viewItem')}</button><button type="button" role="menuitem" onClick={() => editWorkspaceEntryPage(workspaceMenu.entry)}><Pencil /> {t('workspace.editItemPage')}</button></>}
        <button type="button" role="menuitem" onClick={() => renameWorkspaceEntry(workspaceMenu.entry)}><Pencil /> {t('workspace.renameItem')}</button>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteWorkspaceEntry(workspaceMenu.entry)}><Trash2 /> {t('workspace.deleteItem')}</button>
      </div>}

      {mode === 'chat' && <>
        <ChatMain sessions={sessions} activeSessionDetail={activeSessionDetail} activeSessionId={activeSessionId} messages={messages} showReasoning={showReasoning} setShowReasoning={setShowReasoning} hasOlder={hasOlder} hasNewer={hasNewer} loadingMessages={loadingMessages} loadMessageWindow={loadMessageWindow} attachments={attachments} setAttachments={setAttachments} input={input} setInput={setInput} onFiles={onFiles} fileInput={fileInput} sendMessage={sendMessage} composerEnterMode={composerEnterMode} model={model} setModel={changeSessionModel} models={models} effort={effort} setEffort={setEffort} busy={busy} followUpQueue={followUpQueue} onSteerQueuedItem={steerQueuedItem} onEditQueuedItem={editQueuedItem} onReorderQueuedItem={reorderQueuedItem} reconnect={() => { loadModels(); loadSessions(filter); }} chatScrollRef={chatScrollRef} composerRef={composerRef} composerCompact={composerCompact} setComposerCompact={setComposerCompact} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} newMessageCount={newMessageCount} onClearNewMessages={() => { setNewMessageCount(0); if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; }} />
        <WorkspaceAside rootEntries={workspaceTree[''] || workspaceEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openWorkspaceEntry={openWorkspaceEntry} downloadEntry={downloadEntry} preview={preview} setPreview={setPreview} collapsed={workspaceCollapsed} setCollapsed={setWorkspaceCollapsed} openWorkspaceMenu={openWorkspaceMenu} />
      </>}
      {mode === 'images' && <ImageBrowser theme={theme} setTheme={setTheme} requestConfirm={requestConfirm} initialImageFilename={initialImageFilename} writeHashRoute={writeHashRoute} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'workspace' && <WorkspaceMain preview={preview} setPreview={setPreview} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'skills' && <>
        <SkillMain skill={selectedSkill} preview={skillPreview} setPreview={setSkillPreview} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />
        <SkillWorkspaceAside skill={selectedSkill} skillFileTree={skillFileTree} expandedSkillPaths={expandedSkillPaths} toggleSkillFolder={toggleSkillFolder} openSkillFile={openSkillFile} />
      </>}
      {mode === 'cron' && <CronMain name={cronName} setName={setCronName} schedule={cronSchedule} setSchedule={setCronSchedule} prompt={cronPrompt} setPrompt={setCronPrompt} script={cronScript} setScript={setCronScript} deliver={cronDeliver} editingId={cronEditingId} saveCronJob={saveCronJob} runCronJob={runCronJob} deleteCronJob={deleteCronJob} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'memory' && <AdminMain mode={mode} apiBase={apiBase} headers={headers} setStatus={setStatus} theme={theme} setTheme={setTheme} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'insights' && <InsightsMain insights={usageInsights} loading={usageLoading} error={usageError} period={usagePeriod} setPeriod={setUsagePeriod} metric={usageMetric} setMetric={setUsageMetric} refresh={loadUsageInsights} theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'settings' && <SettingsMain apiBase={apiBase} setApiBase={setApiBase} apiKey={apiKey} setApiKey={setApiKey} loadModels={loadModels} loadSessions={loadSessions} theme={theme} setTheme={setTheme} lang={lang} setLang={setLangState} followUpBehaviour={followUpBehaviour} setFollowUpBehaviour={setFollowUpBehaviour} composerEnterMode={composerEnterMode} setComposerEnterMode={setComposerEnterMode} />}
      <CustomDialog dialog={dialog} setDialog={setDialog} />
      <nav className="mobile-bottom-nav" aria-label="Mobile navigation">
        <button className={`rail-btn nav-chat ${mode === 'chat' ? 'active' : ''}`} onClick={() => setNavMode('chat')} aria-label="Chat"><MessageSquare /></button>
        <button className={`rail-btn nav-cron ${mode === 'cron' ? 'active' : ''}`} onClick={() => setNavMode('cron')} aria-label="Cron"><CalendarClock /></button>
        <button className={`rail-btn nav-skills ${mode === 'skills' ? 'active' : ''}`} onClick={() => setNavMode('skills')} aria-label="Skills"><Star /></button>
        <button className={`rail-btn nav-insights ${mode === 'insights' ? 'active' : ''}`} onClick={() => setNavMode('insights', true)} aria-label="Insights"><LineChart /></button>
        <button className={`rail-btn nav-images ${mode === 'images' ? 'active' : ''}`} onClick={() => setNavMode('images', true)} aria-label="Images"><ImageIcon /></button>
        <button className={`rail-btn nav-memory ${mode === 'memory' ? 'active' : ''}`} onClick={() => setNavMode('memory')} aria-label="Memory"><Brain /></button>
      </nav>
    </div>
  );
}

function CustomDialog({ dialog, setDialog }: { dialog: DialogState; setDialog: (dialog: DialogState) => void }) {
  const [value, setValue] = useState('');
  const finish = useCallback((result: string | boolean | null) => { if (dialog) { dialog.resolve(result); setDialog(null); } }, [dialog, setDialog]);
  useEffect(() => { setValue(dialog?.value || ''); }, [dialog]);
  useEffect(() => {
    if (!dialog) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); finish(dialog.variant === 'confirm' ? false : null); }
      if (e.key === 'Enter') { e.preventDefault(); finish(dialog.variant === 'prompt' ? (document.querySelector<HTMLInputElement>('.dialog-card input')?.value ?? '') : true); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [dialog, finish]);
  if (!dialog) return null;
  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) finish(dialog.variant === 'confirm' ? false : null); }}>
    <form className="dialog-card" role="dialog" aria-modal="true" aria-label={dialog.title} onSubmit={(event) => { event.preventDefault(); }}>
      <h2>{dialog.title}</h2>
      <p>{dialog.message}</p>
      {dialog.variant === 'prompt' && <input autoFocus value={value} onChange={(event) => setValue(event.target.value)} />}
      <div className="dialog-actions">
        <button type="button" onClick={() => finish(dialog.variant === 'confirm' ? false : null)}>Cancel</button>
        <button type="button" className={dialog.danger ? 'danger' : ''} onClick={() => finish(dialog.variant === 'prompt' ? value : true)}>{dialog.variant === 'prompt' ? 'Save' : 'Confirm'}</button>
      </div>
    </form>
  </div>;
}
function ThemeCard({ theme, setTheme }: { theme: Theme; setTheme: (v: Theme) => void }) {
  return <div className="theme-card"><div className="theme-title"><span>Appearance</span><span>{themeLabel(theme)}</span></div><label><span>Theme</span><select value={theme} onChange={(e) => setTheme(e.target.value as Theme)}>{THEME_OPTIONS.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}</select></label></div>;
}
function MobileHeaderDrawerButton({ open, onClick }: { open: boolean; onClick: () => void }) {
  return <button type="button" className="mobile-header-drawer rail-btn" aria-label="Open list" aria-expanded={open} onClick={onClick}><List /></button>;
}
function HeaderThemeControl({ theme, setTheme, mode, onNavigateToSettings }: { theme: Theme; setTheme: (v: Theme) => void; mode?: Mode; onNavigateToSettings?: () => void }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => { if (!rootRef.current?.contains(event.target as Node)) setOpen(false); };
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === 'Escape') setOpen(false); };
    window.addEventListener('pointerdown', closeOutside);
    window.addEventListener('keydown', closeOnEscape);
    return () => { window.removeEventListener('pointerdown', closeOutside); window.removeEventListener('keydown', closeOnEscape); };
  }, [open]);
  const isSettingsActive = mode === 'settings';
  return <div className="header-theme-control" ref={rootRef}>
    {onNavigateToSettings && <button type="button" className={`mobile-header-settings-btn rail-btn nav-settings ${isSettingsActive ? 'active' : ''}`} aria-label="Settings" onClick={onNavigateToSettings}><Settings /></button>}
    <button type="button" className="mobile-icon-only palette-btn desktop-only-theme" aria-label="Theme" aria-expanded={open} onClick={() => setOpen((value) => !value)}><Palette /></button>
    {open && <div className="theme-menu" role="menu">
      {THEME_OPTIONS.map((item) => <button key={item.id} type="button" role="menuitemradio" aria-checked={theme === item.id} className={theme === item.id ? 'active' : ''} onClick={() => { setTheme(item.id); setOpen(false); }}><span>{item.label}</span></button>)}
    </div>}
  </div>;
}
function ModeSidebar({ mode }: { mode: Mode }) {
  const label = mode === 'cron' ? 'Cron jobs' : mode === 'memory' ? 'Memory' : mode === 'insights' ? 'Insights' : mode === 'images' ? 'Images' : mode === 'workspace' ? 'Workspace' : 'Settings';
  const text = mode === 'memory' ? 'Edit MEMORY.md and USER.md in a full-width editor.' : mode === 'insights' ? 'Recent model usage, cache, and cost trends.' : mode === 'workspace' ? 'Browse and preview local workspace files.' : mode === 'settings' ? 'API, connection, and WebUI options.' : mode === 'images' ? 'Native image gallery.' : 'Create and manage scheduled jobs.';
  return <div className="admin-side"><h2>{label}</h2><p>{text}</p></div>;
}

function InsightsMain(props: { insights: UsageInsights | null; loading: boolean; error: string; period: 1 | 7 | 30; setPeriod: (value: 1 | 7 | 30) => void; metric: UsageMetric; setMetric: (value: UsageMetric) => void; refresh: () => void; theme: Theme; setTheme: (value: Theme) => void; mode: Mode; onNavigateToSettings: () => void }) {
  const periodData = props.insights?.periods?.find((item) => item.days === props.period);
  const totals = periodData?.totals || emptyTotals();
  const models = useMemo(() => (props.insights?.models || [])
    .map((model) => ({ ...model, periodTotals: finalizeTotals(modelPeriodTotals(model, props.period)) }))
    .filter((model) => model.periodTotals.total_tokens > 0)
    .sort((a, b) => b.periodTotals.total_tokens - a.periodTotals.total_tokens)
    .slice(0, 6), [props.insights, props.period]);
  const topModel = models[0];
  const activeDays = periodSlice(props.insights?.daily || [], props.period);
  const periodLabel = `${props.period}d`;
  const showSkeleton = props.loading;
  const fmtCost = (value: number | undefined) => fmtMoney(value);
  const costMetricLabel = metricLabels.cost_usd;
  return <main className={`main-panel insights-main ${showSkeleton ? 'insights-loading' : ''}`}>
    <header className="chat-header header-no-drawer insights-header">
      <div><h1>Insights</h1><span>{showSkeleton ? 'Loading usage…' : props.error || `Last ${periodLabel} · ${fmtTokens(totals.total_tokens)} tokens`}</span></div>
      <div className="header-actions"><button className="icon-btn mobile-icon-only insights-refresh" onClick={props.refresh} disabled={props.loading} title="Refresh usage"><RefreshCw /></button><HeaderThemeControl theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings} /></div>
    </header>
    <section className="insights-content">
      <div className="insights-toolbar" aria-label="Usage controls">
        <div className="segmented">{([30, 7, 1] as const).map((days) => <button key={days} className={props.period === days ? 'active' : ''} onClick={() => props.setPeriod(days)}>{days}d</button>)}</div>
        <select aria-label="Usage metric" value={props.metric} onChange={(event) => props.setMetric(event.target.value as UsageMetric)}>{(Object.keys(metricLabels) as UsageMetric[]).map((metric) => <option key={metric} value={metric}>{metricLabels[metric]}</option>)}</select>
      </div>
      <div className="insights-cards">
        {showSkeleton ? <>
          <InsightCardSkeleton label="Tokens" />
          <InsightCardSkeleton label="Cache hit" />
          <InsightCardSkeleton label="Cost" />
          <InsightCardSkeleton label="Top model" />
        </> : <>
          <InsightCard label="Tokens" value={fmtTokens(totals.total_tokens)} detail={`${fmtTokens(totals.input)} in · ${fmtTokens(totals.output)} out`} />
          <InsightCard label="Cache hit" value={fmtPercent(totals.cache_hit_rate)} detail={`${fmtTokens(totals.cache_read)} read · ${fmtTokens(totals.cache_write)} write`} />
          <InsightCard label={costMetricLabel} value={fmtCost(totals.cost_usd || totals.actual_cost_usd || totals.estimated_cost_usd)} detail={totals.unpriced_tokens ? `${fmtTokens(totals.unpriced_tokens)} unpriced · ${totals.api_calls || 0} API calls` : `${totals.sessions || 0} sessions · ${totals.api_calls || 0} API calls`} />
          <InsightCard label="Top model" value={topModel ? fmtTokens(topModel.periodTotals.total_tokens) : '—'} detail={topModel?.model || 'No usage'} />
        </>}
      </div>
      <section className="insights-chart-card">
        <div className="insights-card-head"><div><h2>{metricLabels[props.metric]} by model</h2><p>Recent {periodLabel} trend with cache/input/output usage</p></div><LineChart /></div>
        {showSkeleton ? <UsageChartSkeleton /> : <UsageAreaChart days={activeDays} models={models} metric={props.metric} />}
      </section>
      <div className="insights-grid">
        <section className="insights-panel"><h2>Models</h2>{showSkeleton ? <ModelUsageSkeletonList /> : models.length ? models.map((model, index) => <ModelUsageRow key={model.model} model={model} rank={index + 1} />) : <p className="insights-empty">No model usage in this window.</p>}</section>
        <section className="insights-panel"><h2>Other signals</h2>{showSkeleton ? <SignalSkeletonList /> : <><SignalRow name="Reasoning" value={fmtTokens(totals.reasoning)} /><SignalRow name="Tools" value={`${totals.tool_calls || 0}`} /><SignalRow name="Avg/session" value={fmtTokens(totals.avg_tokens_per_session)} /><SourceSignalList sources={(props.insights?.sources || []).slice(0, 6)} /></>}</section>
      </div>
    </section>
  </main>;
}
function InsightCard({ label, value, detail }: { label: string; value: string; detail: string }) {
  return <article className="insight-card"><span>{label}</span><strong>{value}</strong><p>{detail}</p></article>;
}
function InsightCardSkeleton({ label }: { label: string }) {
  return <article className="insight-card insight-card-skeleton" aria-busy="true" aria-label={`${label} loading`}><span>{label}</span><strong><i className="skeleton-block skeleton-number" /></strong><p><i className="skeleton-block skeleton-detail" /></p></article>;
}
function UsageChartSkeleton() {
  return <div className="usage-chart usage-chart-loading" aria-busy="true" aria-label="Loading chart"><div className="chart-loading-grid" aria-hidden="true">{[0, 1, 2, 3].map((item) => <span key={item} />)}</div><div className="chart-loading-line" aria-hidden="true" /><div className="chart-loading-line secondary" aria-hidden="true" /><div className="chart-loading-badge">Loading</div></div>;
}
function ModelUsageSkeletonList() {
  return <div className="model-skeleton-list" aria-busy="true">{[0, 1, 2, 3, 4].map((item) => <article className="model-usage-row model-usage-skeleton" key={item}><div><b><i className="skeleton-block skeleton-rank" /></b><span><i className="skeleton-block skeleton-title" /></span></div><strong><i className="skeleton-block skeleton-value" /></strong><p><i className="skeleton-block skeleton-detail" /></p><div className="model-bar skeleton-bar"><i /></div></article>)}</div>;
}
function SignalSkeletonList() {
  return <>{[0, 1, 2, 3].map((item) => <div className="signal-row signal-skeleton" key={item}><span><i className="skeleton-block skeleton-label" /></span><strong><i className="skeleton-block skeleton-value" /></strong></div>)}</>;
}
function SignalRow({ name, value }: { name: string; value: string }) {
  return <div className="signal-row"><span>{name}</span><strong>{value}</strong></div>;
}
function SourceSignalList({ sources }: { sources: UsageSource[] }) {
  return <div className="signal-row signal-row-sources"><span>Sources</span><div className="source-channel-list">{sources.length ? sources.map((item) => <span className="source-channel-chip" key={item.source}><b>{item.source}</b><em>{fmtTokens(item.totals.total_tokens)}</em></span>) : <span className="source-channel-empty">—</span>}</div></div>;
}
function ModelUsageRow({ model, rank }: { model: UsageModel & { periodTotals: UsageTotals }; rank: number }) {
  const max = Math.max(1, model.periodTotals.total_tokens);
  const cache = Math.min(100, Math.round((model.periodTotals.cache_read / max) * 100));
  return <article className="model-usage-row"><div><b>#{rank}</b><span title={model.model}>{model.model}</span></div><div className="model-value"><strong>{fmtTokens(model.periodTotals.total_tokens)}</strong><small className="model-cost-sub">{fmtMoney(model.periodTotals.cost_usd)}</small></div><p>{fmtTokens(model.periodTotals.input)} input · {fmtTokens(model.periodTotals.output)} output · {fmtPercent(model.periodTotals.cache_hit_rate)} cache</p><div className="model-bar"><i style={{ width: `${cache}%` }} /></div></article>;
}
function UsageAreaChart({ days, models, metric }: { days: UsageDay[]; models: Array<UsageModel & { periodTotals: UsageTotals }>; metric: UsageMetric }) {
  const width = 720;
  const height = 260;
  const pad = { top: 14, right: 18, bottom: 28, left: 58 };
  const compactAxisLabels = useMediaQuery('(max-width: 760px)');
  const series = models.slice(0, 4).map((model, index) => ({ model: model.model, index, values: days.map((day) => metricValue(model.daily.find((item) => item.date === day.date) || day, metric)) }));
  const totalValues = days.map((day) => metricValue(day, metric));
  const allValues = [...totalValues, ...series.flatMap((item) => item.values)];
  const maxValue = Math.max(1, ...allValues);
  const yTicks = chartYAxisTicks(allValues, 4, (value) => metric === 'cost_usd' ? formatMetricValue(metric, value) : compactAxisLabels ? fmtCompactAxisTick(value) : formatMetricValue(metric, value));
  return <div className="usage-chart" data-series-count={series.length}>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label="Usage trend chart" preserveAspectRatio="none">
      <defs>{series.map((item) => <linearGradient key={item.model} id={`insight-grad-${item.index}`} x1="0" x2="0" y1="0" y2="1"><stop offset="0%" stopColor={`var(--chart-${item.index})`} stopOpacity=".52" /><stop offset="100%" stopColor={`var(--chart-${item.index})`} stopOpacity=".03" /></linearGradient>)}</defs>
      <g className="chart-grid" aria-hidden="true">{yTicks.map((tick, tickIndex) => { const y = chartPoint(0, tick.value, 1, width, height, pad, maxValue).y; return <line key={`${tickIndex}-${tick.label}`} x1={pad.left} x2={width - pad.right} y1={y} y2={y} />; })}</g>
      <path className="usage-total-area" d={areaPath(totalValues, width, height, pad, maxValue)} />
      {series.map((item) => <g key={item.model} className={`usage-series usage-series-${item.index}`}>
        <path className="usage-area" d={areaPath(item.values, width, height, pad, maxValue)} fill={`url(#insight-grad-${item.index})`} />
        <path className="usage-line" d={linePath(item.values, width, height, pad, maxValue)} />
      </g>)}
    </svg>
    <div className="chart-y-axis" aria-hidden="true">{yTicks.map((tick, tickIndex) => <span key={`${tickIndex}-${tick.label}`} style={{ top: `${tick.pct}%` }}>{tick.label}</span>)}</div>
    <div className="chart-points">{series.map((item) => item.values.map((value, pointIndex) => { const day = days[pointIndex]; const point = chartPoint(pointIndex, value, item.values.length, width, height, pad, maxValue); const label = chartTooltipLabel(item.model, day?.label || '', value, metricLabels[metric], formatMetricValue(metric, value)); return <span key={`${item.model}-${day?.date || pointIndex}`} className="chart-point-hit" tabIndex={0} aria-label={label} style={{ left: `${(point.x / width) * 100}%`, top: `${(point.y / height) * 100}%`, '--point-color': `var(--chart-${item.index})` } as React.CSSProperties}><span className="chart-tooltip" aria-hidden="true">{label}</span></span>; }))}</div>
    <div className="chart-axis">{days.map((day, index) => <span key={day.date} style={{ left: `${days.length === 1 ? 50 : (index / (days.length - 1)) * 100}%` }}>{index === 0 || index === days.length - 1 || days.length <= 7 ? day.label : ''}</span>)}</div>
    <div className="chart-legend">{series.map((item) => <span key={item.model}><i style={{ background: `var(--chart-${item.index})` }} />{item.model}</span>)}</div>
  </div>;
}
function ChatSidebar(props: { filter: string; setFilter: (v: string) => void; startDraftSession: () => void; pinnedSessions: Session[]; normalSessions: Session[]; activeSessionId: string; setActiveSessionId: (v: string) => void; writeHashRoute: (route: HashRoute) => void; closeMobileSidebar: () => void; pinnedIds: Set<string>; togglePin: (id: string) => void; openSessionMenu: (session: Session, event: React.MouseEvent) => void; openSessionMenuAt: (session: Session, x: number, y: number) => void }) {
  const activateSession = (id: string) => { props.setActiveSessionId(id); props.writeHashRoute({ mode: 'chat', sessionId: id }); buildHashRoute({ mode: 'chat', sessionId: id }); props.closeMobileSidebar(); };
  return <><div className="session-searchbar"><button className="new-chat-btn" aria-label={t('chat.new')} title={t('chat.new')} onClick={() => { props.startDraftSession(); props.closeMobileSidebar(); }}><Plus /></button><input className="filter" placeholder={t('chat.search')} value={props.filter} onChange={(e) => props.setFilter(e.target.value)} /></div><div className="sessions">{props.pinnedSessions.length > 0 && <div className="section-label"><ChevronRight /> {t('chat.pinned')}</div>}{props.pinnedSessions.map((s) => <SessionRow key={s.id} session={s} active={s.id === props.activeSessionId} pinned={props.pinnedIds.has(s.id)} onClick={() => activateSession(s.id)} onTogglePin={() => props.togglePin(s.id)} onContextMenu={(event) => props.openSessionMenu(s, event)} onLongPress={(x, y) => props.openSessionMenuAt(s, x, y)} />)}<div className="section-label"><ChevronRight /> {t('chat.recent')}</div>{props.normalSessions.map((s) => <SessionRow key={s.id} session={s} active={s.id === props.activeSessionId} pinned={props.pinnedIds.has(s.id)} onClick={() => activateSession(s.id)} onTogglePin={() => props.togglePin(s.id)} onContextMenu={(event) => props.openSessionMenu(s, event)} onLongPress={(x, y) => props.openSessionMenuAt(s, x, y)} />)}</div></>;
}
function SessionRow({ session, active, pinned, onClick, onTogglePin, onContextMenu, onLongPress }: { session: Session; active: boolean; pinned: boolean; onClick: () => void; onTogglePin: () => void; onContextMenu: (event: React.MouseEvent) => void; onLongPress: (x: number, y: number) => void }) {
  const longPress = useLongPressContextMenu(onLongPress);
  const leadingIcon = session.source === 'cron' ? <CalendarClock /> : pinned ? <Star /> : null;
  return <div className={`session-item ${active ? 'active' : ''} ${pinned ? 'pinned' : ''} ${leadingIcon ? 'has-leading-icon' : ''}`} role="button" tabIndex={0} onClick={onClick} onContextMenu={onContextMenu} onPointerDown={longPress.onPointerDown} onPointerMove={longPress.onPointerMove} onPointerUp={longPress.onPointerUp} onPointerCancel={longPress.onPointerCancel} onKeyDown={(e) => { if (e.key === 'Enter') onClick(); }}>{leadingIcon && <span className="session-icon">{leadingIcon}</span>}<span className="session-text"><span className="session-title">{sessionDisplayTitle(session)}</span><span className="session-preview">{session.preview || `${session.message_count || 0} messages`}</span></span><button type="button" className="pin-hit" onClick={(e) => { e.stopPropagation(); onTogglePin(); }} title={pinned ? t('chat.unpin') : t('chat.pin')}>{pinned ? <PinOff /> : <Pin />}</button></div>;
}
function StructuredValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) return <span className="tool-empty">null</span>;
  if (Array.isArray(value)) return <div className="tool-children">{value.map((item, index) => <div className="tool-field" key={index}><span className="tool-key">{index}</span><StructuredValue value={item} /></div>)}</div>;
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>);
    return <div className="tool-children">{entries.map(([key, child]) => <div className="tool-field" key={key}><span className="tool-key">{key}</span><StructuredValue value={child} /></div>)}</div>;
  }
  return <span className={`tool-scalar ${typeof value}`}>{String(value)}</span>;
}

function ToolDetailSection({ title, value }: { title: string; value: unknown }) {
  return <section className="tool-detail-section"><h4>{title}</h4><StructuredValue value={value} /></section>;
}

function getToolIcon(toolName: string): React.ReactNode {
  const name = (toolName || '').toLowerCase().replace(/^functions\./, '');
  if (name.startsWith('browser_')) return <Globe />;
  if (name === 'terminal' || name === 'process') return <Terminal />;
  if (name === 'read_file' || name === 'write_file' || name === 'patch') return <FileText />;
  if (name === 'search_files') return <Search />;
  if (name === 'execute_code') return <Code />;
  if (name === 'web_search' || name === 'x_search') return <Search />;
  if (name === 'web_extract') return <Globe />;
  if (name === 'vision_analyze') return <Eye />;
  if (name === 'image_generate') return <ImageIcon />;
  if (name === 'video_generate' || name === 'video_analyze') return <Video />;
  if (name === 'text_to_speech') return <Volume2 />;
  if (name.startsWith('skill')) return <Puzzle />;
  if (name === 'memory') return <Brain />;
  if (name === 'session_search') return <History />;
  if (name === 'delegate_task') return <Users />;
  if (name === 'cronjob') return <CalendarClock />;
  if (name === 'clarify') return <CircleHelp />;
  if (name === 'send_message') return <Send />;
  if (name === 'todo') return <CheckSquare />;
  if (name.startsWith('kanban_')) return <Layout />;
  if (name.startsWith('ha_')) return <Home />;
  if (name === 'discord' || name === 'discord_admin') return <MessageSquare />;
  if (name.startsWith('feishu_')) return <FileText />;
  if (name.startsWith('yb_')) return <MessageSquare />;
  if (name.startsWith('mcp_')) return <Server />;
  if (name === 'workflow_run') return <Repeat />;
  if (name === 'mixture_of_agents') return <Network />;
  if (name === 'computer_use') return <Globe />;
  return <Settings />;
}

function ToolMessageView({ message }: { message: ChatMessage }) {
  const [expanded, setExpanded] = useState(false);
  const summary = useMemo(() => summarizeToolMessage(message.content, message.toolName, message.toolInput), [message.content, message.toolName, message.toolInput]);
  const toolName = summary.toolName;
  const isError = summary.status !== 'ok';
  return <article className={`msg-row tool${isError ? ' tool-error' : ''}`}>
    <div className="avatar">{getToolIcon(toolName)}</div>
    <div className="msg-content tool-card">
      <button type="button" className="tool-summary" aria-expanded={expanded} onClick={() => setExpanded((value) => !value)}>
        <span className="tool-inline-icon">{getToolIcon(toolName)}</span>
        <span className={`tool-title${isError ? ' err' : ''}`}>{summary.title}</span>
        <span className="tool-subtitle">{summary.subtitle}</span>
        <ChevronRight className={`tool-chevron ${expanded ? 'open' : ''}`} />
      </button>
      {expanded && <div className="tool-detail">
        {summary.input !== undefined && <ToolDetailSection title="Invocation" value={summary.input} />}
        <ToolDetailSection title="Result" value={summary.result} />
      </div>}
    </div>
  </article>;
}

function MessageView({ message, showReasoning = false }: { message: ChatMessage; showReasoning?: boolean }) {
  if (!shouldRenderMessage(message, showReasoning)) return null;
  if (message.role === 'tool') return <ToolMessageView message={message} />;
  const isPending = !!message.pending;
  const fallback = isPending ? '…' : '';
  const html = markdownText(message.content || fallback);
  return (
    <article className={`msg-row ${message.role}${isPending ? ' pending' : ''}`}>
      <div className="avatar">{message.role === 'assistant' ? <Bot /> : <UserRound />}</div>
      <div className="msg-content">
        <div className="msg-meta">
          <span>{roleName(message.role)}</span>
          <time>{message.timestamp ? new Date(Number(message.timestamp) * (Number(message.timestamp) < 1e12 ? 1000 : 1)).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) : new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</time>
          {isPending && <span className="stream-state" aria-label="streaming"><span className="stream-dots"><i /><i /><i /></span><span className="stream-label">streaming</span></span>}
        </div>
        <div className="msg-body">
          <span dangerouslySetInnerHTML={{ __html: html }} />
          {isPending && <span className="stream-caret" aria-hidden="true" />}
        </div>
        {message.reasoning && showReasoning && <section className="msg-reasoning" aria-label="Reasoning / thinking"><span>Thinking</span><pre>{message.reasoning}</pre></section>}
      </div>
    </article>
  );
}

function DropdownControl({ icon, ariaLabel, label = '', value, options, onChange, wide = false, hideLabel = false, searchable = false }: { icon: React.ReactNode; ariaLabel: string; label?: string; value: string; options: Array<{ id: string; label: string; provider?: string }>; onChange: (value: string, option?: { id: string; label: string; provider?: string }) => void; wide?: boolean; hideLabel?: boolean; searchable?: boolean }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const rootRef = useRef<HTMLDivElement>(null);
  const current = options.find((item) => item.id === value) || { id: value, label: value };
  const filteredOptions = searchable && query.trim() ? options.filter((item) => `${item.label} ${item.id}`.toLowerCase().includes(query.trim().toLowerCase())) : options;
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: PointerEvent) => {
      if (rootRef.current?.contains(event.target as Node)) return;
      setOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    window.addEventListener('pointerdown', closeOutside);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('pointerdown', closeOutside);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [open]);
  return <div ref={rootRef} className={`dropdown-control ${wide ? 'wide' : ''} ${searchable ? 'searchable' : ''} ${open ? 'open' : ''}`}>
    <button type="button" className="dropdown-trigger" aria-label={ariaLabel} aria-expanded={open} onClick={() => { setOpen((v) => !v); setQuery(''); }}>
      <span className="dropdown-icon">{icon}</span>
      <span className="dropdown-copy">{!hideLabel && <span className="dropdown-label">{label || ariaLabel}</span>}<span className="dropdown-value">{current.label}</span></span>
      <ChevronRight className="dropdown-caret" />
    </button>
    {open && <div className="dropdown-menu" role="listbox">
      {searchable && <input className="dropdown-search" autoFocus placeholder={t('chat.searchModels')} value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.stopPropagation()} />}
      {filteredOptions.map((item) => <button type="button" role="option" aria-selected={item.id === value} className={item.id === value ? 'selected' : ''} key={item.id} onClick={() => { onChange(item.id, item); setOpen(false); }}>{item.label}</button>)}
      {filteredOptions.length === 0 && <span className="dropdown-empty">{t('chat.noModels')}</span>}
    </div>}
  </div>;
}

function ChatMain(props: any) {
  const active = props.sessions.find((s: Session) => s.id === props.activeSessionId) || props.activeSessionDetail;
  const isMobile = useMediaQuery('(max-width: 760px)');
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const resizeComposerTextarea = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    if (props.composerCompact) {
      textarea.style.height = '';
      textarea.style.maxHeight = '';
      textarea.style.overflowY = '';
      return;
    }
    const minHeight = isMobile ? 64 : 96;
    const maxHeight = Math.max(minHeight, Math.floor(window.innerHeight * 0.2));
    textarea.style.height = 'auto';
    textarea.style.maxHeight = `${maxHeight}px`;
    textarea.style.overflowY = 'hidden';
    const nextHeight = Math.min(Math.max(textarea.scrollHeight, minHeight), maxHeight);
    textarea.style.height = `${nextHeight}px`;
    textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden';
  }, [isMobile, props.composerCompact]);
  useLayoutEffect(() => { resizeComposerTextarea(); }, [props.input, props.composerCompact, resizeComposerTextarea]);
  useEffect(() => {
    window.addEventListener('resize', resizeComposerTextarea);
    return () => window.removeEventListener('resize', resizeComposerTextarea);
  }, [resizeComposerTextarea]);
  const collapseComposerForHistory = () => {
    if (!isMobile) return;
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && props.composerRef.current?.contains(activeElement)) activeElement.blur();
    props.setComposerCompact(true);
  };
  const onScroll = (e: React.UIEvent<HTMLElement>) => {
    const el = e.currentTarget;
    if (isMobile && !props.composerRef.current?.contains(document.activeElement)) props.setComposerCompact(true);
    if (el.scrollTop < 80 && props.hasOlder && !props.loadingMessages) props.loadMessageWindow(props.activeSessionId, 'older');
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 80 && props.hasNewer && !props.loadingMessages) props.loadMessageWindow(props.activeSessionId, 'newer');
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 120 && props.newMessageCount > 0) props.onClearNewMessages();
  };
  const sessionModel = realModelOrEmpty(active?.model) || realModelOrEmpty(props.activeSessionDetail?.model) || realModelOrEmpty(props.model) || props.models[0]?.id || '';
  const currentModel = sessionModel;
  const activeTitle = active?.id === DRAFT_SESSION_ID ? 'New conversation' : active ? sessionDisplayTitle(active) : 'Hermes Agent';
  const headerTimes = sessionHeaderTimes(active, props.messages);
  const currentOption = currentModel ? currentModelDisplayOption(currentModel, props.models) : undefined;
  const modelOptions = currentOption ? [currentOption, ...props.models.filter((m: ModelOption) => m.id !== currentModel)] : props.models;
  const effortOptions = EFFORTS.map((x) => ({ id: x, label: x }));
  return <main className="main-panel">
    <header className="chat-header"><MobileHeaderDrawerButton open={props.mobileSidebarOpen} onClick={props.toggleMobileSidebar} /><div><h1>{activeTitle}</h1><span>{props.messages.length || 0} loaded · {active?.message_count || 0} total</span></div><div className="header-actions"><div className="session-header-times" aria-label="Session times">{headerTimes.started && <time>{headerTimes.started}</time>}{headerTimes.latest && <time>{headerTimes.latest}</time>}</div><HeaderThemeControl theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings} /></div></header>
    <section className="chat-scroll" ref={props.chatScrollRef} onScroll={onScroll} onPointerDown={collapseComposerForHistory} onTouchStart={collapseComposerForHistory} onWheel={collapseComposerForHistory}>
      {props.loadingMessages && <div className="history-loading" aria-live="polite">Loading history…</div>}
      {props.messages.length === 0 && <div className="empty-state chat-empty-state"><Bot className="big-mark" /><h2>{t('chat.inputPlaceholder')}</h2><p>Streaming chat through Hermes API Server. Message history is loaded in pages.</p></div>}
      {(() => {
        const splitIdx = props.newMessageCount > 0 ? Math.max(0, props.messages.length - props.newMessageCount) : -1;
        return props.messages.map((m: ChatMessage, i: number) => (
          <React.Fragment key={m.id}>
            {i === splitIdx && <div className="new-messages-separator" role="separator"><span className="new-messages-label">{t('chat.newMessages')}</span></div>}
            <MessageView message={m} showReasoning={props.showReasoning} />
          </React.Fragment>
        ));
      })()}
    </section>
    <footer className={`composer-wrap ${props.composerCompact ? 'composer-compact' : ''}`} ref={props.composerRef}>
      {props.newMessageCount > 0 && <button className="new-messages-bubble" onClick={props.onClearNewMessages} aria-label={t('chat.newMessages')}>{props.newMessageCount === 1 ? t('chat.newMessageCount') : t('chat.newMessagesCount').replace('{n}', String(props.newMessageCount))}</button>}
      <FollowUpQueueView items={props.followUpQueue || []} onSteer={props.onSteerQueuedItem} onEdit={props.onEditQueuedItem} onReorder={props.onReorderQueuedItem} />
      <div className="attachments">{props.attachments.map((a: Attachment) => <span className={`att ${a.kind}`} key={a.id}>{a.kind === 'image' ? <ImageIcon /> : <FileText />} {a.name} <button onClick={() => props.setAttachments((old: Attachment[]) => old.filter((x) => x.id !== a.id))}><X /></button></span>)}</div>
      <div className="composer-box" onDragOver={(e) => e.preventDefault()} onDrop={(e) => { e.preventDefault(); props.onFiles(e.dataTransfer.files); }}>
        <textarea ref={textareaRef} value={props.input} onFocus={() => props.setComposerCompact(false)} onChange={(e) => props.setInput(e.target.value)} placeholder={t('chat.inputPlaceholder')} onKeyDown={(e) => { if (e.key !== 'Enter' || e.shiftKey || (e.nativeEvent as KeyboardEvent).isComposing) return; const modified = e.metaKey || e.ctrlKey; const shouldSend = props.composerEnterMode === 'enter-newline' ? modified : !modified; if (!shouldSend) return; e.preventDefault(); props.sendMessage(); }} />
        <div className="composer-footer">
          <input ref={props.fileInput} type="file" multiple hidden onChange={(e) => props.onFiles(e.target.files)} />
          <button className="icon-btn attach-btn" onClick={() => props.fileInput.current?.click()} title={t('chat.attachFiles')}><Paperclip /></button>
          <DropdownControl icon={<Bot />} ariaLabel="Model" value={currentModel} options={modelOptions} onChange={props.setModel} wide hideLabel searchable />
          <DropdownControl icon={<Brain />} ariaLabel="Reasoning" value={props.effort} options={effortOptions} onChange={props.setEffort} hideLabel />
          <button type="button" className={`icon-btn reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`} aria-pressed={props.showReasoning} aria-label={props.showReasoning ? t('chat.hideThinking') : t('chat.showThinking')} title={props.showReasoning ? t('chat.hideThinking') : t('chat.showThinking')} onClick={() => props.setShowReasoning(!props.showReasoning)}><Lightbulb /></button>
          <button className="send-btn mobile-icon-only" onClick={props.sendMessage} aria-label={props.busy ? 'Queue follow-up' : 'Send'}><Send /> <span className="btn-label">{props.busy ? 'Queue' : 'Send'}</span></button>
        </div>
      </div>
    </footer>
  </main>;
}

function FollowUpQueueView({ items, onSteer, onEdit, onReorder }: { items: FollowUpQueueItem[]; onSteer: (item: FollowUpQueueItem) => void; onEdit: (item: FollowUpQueueItem) => void; onReorder: (fromIndex: number, toIndex: number) => void }) {
  const dragIdx = React.useRef<number | null>(null);
  const [dragOverIdx, setDragOverIdx] = React.useState<number | null>(null);
  if (!items.length) return null;
  return <div className="followup-queue" aria-label="Queued follow-ups">
    {items.map((item, index) => <div
      className={`followup-item${dragOverIdx === index ? ' drag-over' : ''}`}
      key={item.id}
      title={item.text}
      onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = 'move'; setDragOverIdx(index); }}
      onDragLeave={() => { setDragOverIdx((prev) => prev === index ? null : prev); }}
      onDrop={(e) => { e.preventDefault(); const from = dragIdx.current; if (from !== null && from !== index) onReorder(from, index); dragIdx.current = null; setDragOverIdx(null); }}
    >
      <span
        className="followup-drag-handle"
        draggable={true}
        onDragStart={(e) => { dragIdx.current = index; e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(index)); (e.target as HTMLElement).closest('.followup-item')?.classList.add('dragging'); }}
        onDragEnd={() => { dragIdx.current = null; setDragOverIdx(null); document.querySelectorAll('.followup-item.dragging').forEach((el) => el.classList.remove('dragging')); }}
      ><GripVertical /></span>
      <span className="followup-text">{item.text}</span>
      <button type="button" className="followup-action" onClick={() => onSteer(item)} title="Steer now">Steer</button>
      <button type="button" className="followup-action" onClick={() => onEdit(item)} title="Edit queued follow-up"><Pencil /></button>
    </div>)}
  </div>;
}

function WorkspaceAside(props: any) {
  if (props.collapsed) return <aside className="workspace workspace-collapsed"><div className="workspace-collapsed-actions"><button className="workspace-rail-btn" title={t('workspace.expand')} aria-label="Expand workspace" onClick={() => props.setCollapsed(false)}><ChevronLeft /></button><button className="workspace-rail-btn" title={t('workspace.openPage')} aria-label="Open workspace page" onClick={() => { window.location.hash = '#/workspace'; }}><Folder /></button></div></aside>;
  return <aside className="workspace"><WorkspaceBrowser rootEntries={props.rootEntries} workspaceTree={props.workspaceTree} expandedWorkspacePaths={props.expandedWorkspacePaths} toggleWorkspaceFolder={props.toggleWorkspaceFolder} openWorkspaceEntry={props.openWorkspaceEntry} downloadEntry={props.downloadEntry} preview={props.preview} setPreview={props.setPreview} compact setCollapsed={props.setCollapsed} openWorkspaceMenu={props.openWorkspaceMenu} /></aside>;
}
function WorkspaceMain({ preview, setPreview, theme, setTheme, mobileSidebarOpen, toggleMobileSidebar, mode, onNavigateToSettings }: any) {
  return <main className="main-panel workspace-main"><header className="chat-header"><MobileHeaderDrawerButton open={mobileSidebarOpen} onClick={toggleMobileSidebar} /><div><h1>{t('workspace.title')}</h1><span>{t('workspace.editor')}</span></div><HeaderThemeControl theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><WorkspaceEditorPreview preview={preview} setPreview={setPreview} /></main>;
}
function WorkspaceSidebar({ rootEntries, workspaceTree, expandedWorkspacePaths, toggleWorkspaceFolder, openWorkspaceEntry, downloadEntry, openWorkspaceMenu }: any) {
  const renderRows = (entries: WorkspaceEntry[], depth = 0): React.ReactNode => entries.map((entry) => {
    const expanded = entry.kind === 'dir' && expandedWorkspacePaths.has(entry.path);
    const children = expanded ? (workspaceTree[entry.path] || []) : [];
    return <React.Fragment key={entry.path}>
      <div className={`file-row workspace-tree-row ${entry.kind} ${expanded ? 'expanded' : ''}`} style={{ paddingLeft: 10 + depth * 16 }} role="button" tabIndex={0} onClick={() => entry.kind === 'dir' ? toggleWorkspaceFolder(entry) : openWorkspaceEntry(entry)} onContextMenu={(ev) => openWorkspaceMenu?.(entry, ev)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); entry.kind === 'dir' ? toggleWorkspaceFolder(entry) : openWorkspaceEntry(entry); } }}>
        <span className="caret">{entry.kind === 'dir' ? (expanded ? <ChevronDown /> : <ChevronRight />) : null}</span>{entry.kind === 'dir' ? <Folder /> : <FileText />}<span className="file-name">{entry.name}</span><span className="file-size">{entry.kind === 'file' ? fmtSize(entry.size) : ''}</span>{entry.kind === 'file' && <button title="download" onClick={(ev) => { ev.stopPropagation(); downloadEntry(entry); }}><Download /></button>}
      </div>
      {expanded && children.length > 0 && renderRows(children, depth + 1)}
    </React.Fragment>;
  });
  return <><div className="workspace-sidebar-head"><div><h2>Workspace</h2><p>File tree</p></div></div><div className="workspace-tree file-list">{renderRows(rootEntries || [])}</div></>;
}
function SkillsSidebar({ skills, activeSkillName, selectSkill, toggleSkillEnabled, filter, setFilter, expandedCats, setExpandedCats, closeMobileSidebar }: { skills: Skill[]; activeSkillName: string; selectSkill: (skill: Skill) => void; toggleSkillEnabled: (skill: Skill, enabled: boolean) => void; filter: string; setFilter: (v: string) => void; expandedCats: Set<string>; setExpandedCats: (v: Set<string>) => void; closeMobileSidebar: () => void }) {
  const grouped = skills.reduce<Record<string, Skill[]>>((acc, skill) => { const cat = skill.category || 'uncategorized'; if (cat === '.archive') return acc; (acc[cat] ||= []).push(skill); return acc; }, {});
  const cats = Object.keys(grouped).sort();
  const query = filter.trim().toLowerCase();
  const filteredCats = query ? cats.filter((cat) => grouped[cat].some((s) => s.name.toLowerCase().includes(query) || (s.description || '').toLowerCase().includes(query))) : cats;
  const filteredSkills = (cat: string) => query ? grouped[cat].filter((s) => s.name.toLowerCase().includes(query) || (s.description || '').toLowerCase().includes(query)) : grouped[cat];
  const toggleCat = (cat: string) => setExpandedCats(new Set(expandedCats.has(cat) ? [...expandedCats].filter((c) => c !== cat) : [...expandedCats, cat]));
  return <><div className="cron-sidebar-head"><div><h2>Skills</h2><p>{skills.length} {t('skills.installed')}</p></div></div><div className="session-searchbar"><input className="filter" placeholder={t('skills.search')} value={filter} onChange={(e) => setFilter(e.target.value)} style={{ gridColumn: '1 / -1' }} /></div><div className="skills-list sessions">{filteredCats.map((cat) => <React.Fragment key={cat}><div className="section-label" role="button" tabIndex={0} onClick={() => toggleCat(cat)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); toggleCat(cat); } }}>{expandedCats.has(cat) ? <ChevronDown /> : <ChevronRight />} {cat}</div>{expandedCats.has(cat) && filteredSkills(cat).map((skill) => <button type="button" className={`skill-row session-item ${skill.name === activeSkillName ? 'active' : ''}`} key={skill.name} onClick={() => { selectSkill(skill); closeMobileSidebar(); }}><span className="session-text"><span className="session-title">{skill.name}</span><span className="session-preview">{skill.description || t('skills.noDescription')}</span></span><span className="skill-enable-toggle" role="switch" aria-checked={skill.enabled !== false} tabIndex={0} onClick={(ev) => { ev.stopPropagation(); toggleSkillEnabled(skill, !(skill.enabled !== false)); }} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); ev.stopPropagation(); toggleSkillEnabled(skill, !(skill.enabled !== false)); } }} /></button>)}</React.Fragment>)}</div></>;
}
function SkillMain({ skill, preview, setPreview, theme, setTheme, mobileSidebarOpen, toggleMobileSidebar, mode, onNavigateToSettings }: { skill: Skill | null; preview: any; setPreview: (value: any) => void; theme: Theme; setTheme: (v: Theme) => void; mobileSidebarOpen: boolean; toggleMobileSidebar: () => void; mode: Mode; onNavigateToSettings: () => void }) {
  return <main className="main-panel skills-main"><header className="chat-header"><MobileHeaderDrawerButton open={mobileSidebarOpen} onClick={toggleMobileSidebar} /><div><h1>{skill?.name || 'Skills'}</h1><span>{skill?.description || t('skills.select')}</span></div><HeaderThemeControl theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><WorkspaceEditorPreview preview={preview} setPreview={setPreview} emptyIcon={Puzzle} emptyTitle={t('skills.select')} emptyDesc={t('skills.selectHint')} /></main>;
}
function SkillWorkspaceAside({ skill, skillFileTree, expandedSkillPaths, toggleSkillFolder, openSkillFile }: { skill: Skill | null; skillFileTree: Record<string, WorkspaceEntry[]>; expandedSkillPaths: Set<string>; toggleSkillFolder: (entry: WorkspaceEntry) => void; openSkillFile: (skillName: string, path: string) => void }) {
  const renderRows = (entries: WorkspaceEntry[], depth = 0): React.ReactNode => entries.filter((e) => e.name !== '.archive').map((entry) => {
    const expanded = entry.kind === 'dir' && expandedSkillPaths.has(entry.path);
    const children = expanded ? (skillFileTree[entry.path] || []) : [];
    return <React.Fragment key={entry.path}>
      <div className={`file-row workspace-tree-row ${entry.kind} ${expanded ? 'expanded' : ''}`} style={{ paddingLeft: 10 + depth * 16 }} role="button" tabIndex={0} onClick={() => entry.kind === 'dir' ? toggleSkillFolder(entry) : skill && openSkillFile(skill.name, entry.path)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); entry.kind === 'dir' ? toggleSkillFolder(entry) : skill && openSkillFile(skill.name, entry.path); } }}>
        <span className="caret">{entry.kind === 'dir' ? (expanded ? <ChevronDown /> : <ChevronRight />) : null}</span>{entry.kind === 'dir' ? <Folder /> : <FileText />}<span className="file-name">{entry.name}</span><span className="file-size">{entry.kind === 'file' ? fmtSize(entry.size) : ''}</span>
      </div>
      {expanded && renderRows(children, depth + 1)}
    </React.Fragment>;
  });
  return <aside className="skill-workspace workspace"><div className="workspace-sidebar-head"><div><h2>{t('skills.skillFiles')}</h2><p>{skill?.category || t('skills.select')}</p></div></div><div className="workspace-tree file-list">{renderRows(skillFileTree[''] || [])}</div></aside>;
}
function WorkspaceEditorPreview({ preview, setPreview, emptyIcon, emptyTitle, emptyDesc }: any) {
  const [editMode, setEditMode] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [saving, setSaving] = useState(false);
  const startEdit = () => { setEditContent(preview.content || ''); setEditMode(true); };
  const cancelEdit = () => { setEditMode(false); setEditContent(''); };
  useEffect(() => { setEditMode(false); setEditContent(''); }, [preview.path]);
  useEffect(() => { if (preview.editRequest && preview.kind === 'text') startEdit(); }, [preview.editRequest]);
  const saveEdit = async () => {
    setSaving(true);
    try {
      const res = await fetch(`/workspace/file?path=${encodeURIComponent(preview.path)}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ content: editContent }) });
      if (!res.ok) { alert(`Save failed: ${res.status}`); return; }
      setPreview({ ...preview, content: editContent });
      setEditMode(false);
      setEditContent('');
    } finally { setSaving(false); }
  };
  if (preview.kind === 'none') { const Icon = emptyIcon || Folder; return <section className="workspace-editor-preview empty"><div className="empty-state"><Icon className="big-mark" /><h2>{emptyTitle || t('workspace.selectFile')}</h2><p>{emptyDesc || t('workspace.selectFileDesc')}</p></div></section>; }
  return <section className="workspace-editor-preview"><div className="preview-head"><span>{basename(preview.path)}</span><div className="preview-head-actions">{!editMode && preview.kind === 'text' && <button className="icon-btn" aria-label="Edit" onClick={startEdit}><Pencil /></button>}{editMode && <><button className="icon-btn" disabled={saving} onClick={saveEdit}><Save /></button><button className="icon-btn" aria-label="Cancel edit" onClick={cancelEdit}><X /></button></>}{!editMode && <button className="icon-btn" aria-label="Close preview" onClick={() => setPreview({ path: '', content: '', kind: 'none' })}><X /></button>}</div></div>{preview.kind === 'image' ? <div className="workspace-image-preview"><img src={preview.url} /></div> : editMode ? <div className="workspace-editor-overlay"><pre className="workspace-code-highlight workspace-editor-highlight" aria-hidden="true" dangerouslySetInnerHTML={{ __html: highlightWorkspaceText(editContent || '', preview.path) + '\n' }} /><textarea className="workspace-editor-textarea" value={editContent} onChange={(e) => setEditContent(e.target.value)} spellCheck={false} onScroll={(e) => { const pre = e.currentTarget.previousElementSibling as HTMLElement; if (pre) { pre.scrollTop = e.currentTarget.scrollTop; pre.scrollLeft = e.currentTarget.scrollLeft; } }} /></div> : <div className="workspace-text-preview"><pre className="workspace-code-highlight" dangerouslySetInnerHTML={{ __html: highlightWorkspaceText(preview.content || '', preview.path) }} /></div>}</section>;
}
function WorkspaceBrowser({ rootEntries, workspaceTree, expandedWorkspacePaths, toggleWorkspaceFolder, openWorkspaceEntry, downloadEntry, preview, setPreview, compact, setCollapsed, openWorkspaceMenu }: any) {
  const renderRows = (entries: WorkspaceEntry[], depth = 0): React.ReactNode => entries.map((entry) => {
    const expanded = entry.kind === 'dir' && expandedWorkspacePaths.has(entry.path);
    const children = expanded ? (workspaceTree[entry.path] || []) : [];
    return <React.Fragment key={entry.path}>
      <div className={`file-row workspace-tree-row ${entry.kind} ${expanded ? 'expanded' : ''}`} style={{ paddingLeft: 10 + depth * 16 }} role="button" tabIndex={0} onClick={() => entry.kind === 'dir' ? toggleWorkspaceFolder(entry) : openWorkspaceEntry(entry, compact ? { route: false } : undefined)} onContextMenu={(ev) => openWorkspaceMenu?.(entry, ev)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); entry.kind === 'dir' ? toggleWorkspaceFolder(entry) : openWorkspaceEntry(entry, compact ? { route: false } : undefined); } }}>
        <span className="caret">{entry.kind === 'dir' ? (expanded ? <ChevronDown /> : <ChevronRight />) : null}</span>{entry.kind === 'dir' ? <Folder /> : <FileText />}<span className="file-name">{entry.name}</span><span className="file-size">{entry.kind === 'file' ? fmtSize(entry.size) : ''}</span>{entry.kind === 'file' && <button title="download" onClick={(ev) => { ev.stopPropagation(); downloadEntry(entry); }}><Download /></button>}
      </div>
      {expanded && children.length > 0 && renderRows(children, depth + 1)}
    </React.Fragment>;
  });
  return <>
    <header className="workspace-head"><span className="panel-title">WORKSPACE</span><span>{compact ? 'MAIN' : 'FULL'}</span><button aria-label={compact ? t('workspace.collapse') : undefined} onClick={() => compact ? setCollapsed(true) : setPreview({ path: '', content: '', kind: 'none' })}><X /></button></header>
    <div className="workspace-tree file-list">{renderRows(rootEntries || [])}</div>
    {preview.kind !== 'none' && <div className="preview"><div className="preview-head"><span>{basename(preview.path)}</span><button onClick={() => setPreview({ path: '', content: '', kind: 'none' })}><X /></button></div>{preview.kind === 'image' ? <img src={preview.url} /> : <pre className="workspace-code-highlight" dangerouslySetInnerHTML={{ __html: highlightWorkspaceText(preview.content || '', preview.path) }} />}</div>}
  </>;
}
function AdminMain({ mode, setStatus, theme, setTheme, onNavigateToSettings }: { mode: Extract<Mode, 'memory'>; apiBase: string; headers: (json?: boolean) => Record<string, string>; setStatus: (v: string) => void; theme: Theme; setTheme: (v: Theme) => void; onNavigateToSettings: () => void }) {
  return <main className={`main-panel admin-main ${mode === 'memory' ? 'memory-main' : ''}`}><header className="chat-header header-no-drawer"><div><h1>{t('memory.title')}</h1><span>{t('memory.subtitle')}</span></div><HeaderThemeControl theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><MemoryPanel setStatus={setStatus} /></main>;
}
function CronSidebar({ jobs, editingId, beginCronEdit, resetCronForm, writeHashRoute, closeMobileSidebar }: { jobs: Job[]; editingId: string; beginCronEdit: (job: Job) => void; resetCronForm: () => void; writeHashRoute: (route: HashRoute) => void; closeMobileSidebar: () => void }) {
  return <><div className="cron-sidebar-head"><div><h2>Cron jobs</h2><p>{jobs.length} scheduled jobs</p></div><button className="new-chat-btn" aria-label={t('cron.newJob')} title={t('cron.newJob')} onClick={() => { resetCronForm(); writeHashRoute({ mode: 'cron' }); closeMobileSidebar(); }}><Plus /></button></div><div className="cron-sidebar-list">{jobs.map((j) => <button type="button" data-route={buildHashRoute({ mode: 'cron', jobId: jobId(j) })} className={`cron-sidebar-row ${jobId(j) === editingId ? 'active' : ''}`} key={jobId(j)} onClick={() => { beginCronEdit(j); closeMobileSidebar(); }}>
    <span className="session-icon"><CalendarClock /></span><span className="session-text"><span className="session-title">{j.name || jobId(j)}</span><span className="session-preview">{jobSchedule(j.schedule)} · {jobState(j)}{j.script ? ` · ${j.script}` : ''}</span></span>
  </button>)}</div></>;
}
function CronMain(props: { name: string; setName: (v: string) => void; schedule: string; setSchedule: (v: string) => void; prompt: string; setPrompt: (v: string) => void; script: string; setScript: (v: string) => void; deliver: string; editingId: string; saveCronJob: () => void; runCronJob: () => void; deleteCronJob: () => void; theme: Theme; setTheme: (v: Theme) => void; mobileSidebarOpen: boolean; toggleMobileSidebar: () => void; mode: Mode; onNavigateToSettings: () => void }) {
  const deliverDisplay = (d: string) => {
    if (!d) return '—';
    if (d === 'origin') return 'origin (reply to chat)';
    if (d === 'local') return 'local only';
    if (d === 'all') return 'all connected channels';
    return d;
  };
  return <main className="main-panel cron-main">
    <header className="chat-header"><MobileHeaderDrawerButton open={props.mobileSidebarOpen} onClick={props.toggleMobileSidebar} /><div><h1>{props.editingId ? t('cron.editCron') : t('cron.newCron')}</h1><span>{t('cron.jobs')}</span></div><HeaderThemeControl theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings} /></header>
    <section className="cron-detail-wrap"><div className="cron-detail">
      <label className="cron-field"><span>Name</span><input value={props.name} onChange={(e) => props.setName(e.target.value)} placeholder="Job name" /></label>
      <label className="cron-field"><span>Schedule</span><input value={props.schedule} onChange={(e) => props.setSchedule(e.target.value)} placeholder="Schedule, e.g. 0 9 * * *" /></label>
      <label className="cron-field cron-prompt"><span>Prompt</span><textarea value={props.prompt} onChange={(e) => props.setPrompt(e.target.value)} placeholder="Prompt" /></label>
      <label className="cron-field cron-script"><span>Script</span><textarea value={props.script} onChange={(e) => props.setScript(e.target.value)} placeholder="Script (optional)" /></label>
      {props.editingId && <label className="cron-field cron-fullwidth"><span>Delivery target</span><input value={deliverDisplay(props.deliver)} readOnly /></label>}
      <div className="cron-detail-actions">
        <button aria-label="save cron job" className="mobile-icon-only" onClick={props.saveCronJob}><Save /> <span className="btn-label">Save</span></button>
        <button aria-label="run cron job" className="mobile-icon-only" disabled={!props.editingId} onClick={props.runCronJob}><PlayMark /> <span className="btn-label">Run</span></button>
        <button aria-label="delete cron job" className="danger mobile-icon-only" disabled={!props.editingId} onClick={props.deleteCronJob}><Trash2 /> <span className="btn-label">Delete</span></button>
      </div>
    </div></section>
  </main>;
}
function MemoryPanel({ setStatus }: { setStatus: (v: string) => void }) {
  const [doc, setDoc] = useState<MemoryDoc>({ memory: '', user: '' });
  const load = useCallback(async () => { try { const res = await fetch('/memory'); if (!res.ok) throw new Error(await res.text()); setDoc(await res.json()); } catch (err: any) { setStatus(`Memory unavailable: ${err.message}`); } }, [setStatus]);
  useEffect(() => { load(); }, [load]);
  const save = async () => { const res = await fetch('/memory', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(doc) }); setStatus(res.ok ? t('memory.saved') : await res.text()); };
  return <section className="admin-content memory-grid"><label><span>MEMORY.md</span><textarea value={doc.memory} onChange={(e) => setDoc({ ...doc, memory: e.target.value })}/></label><label><span>USER.md</span><textarea value={doc.user} onChange={(e) => setDoc({ ...doc, user: e.target.value })}/></label><button className="save-memory" onClick={save}>{t('memory.save')}</button></section>;
}
function SettingsMain(props: { apiBase: string; setApiBase: (v: string) => void; apiKey: string; setApiKey: (v: string) => void; loadModels: () => void; loadSessions: () => void; theme: Theme; setTheme: (v: Theme) => void; lang: Lang; setLang: (v: Lang) => void; followUpBehaviour: FollowUpBehaviour; setFollowUpBehaviour: (v: FollowUpBehaviour) => void; composerEnterMode: ComposerEnterMode; setComposerEnterMode: (v: ComposerEnterMode) => void }) {
  const LANG_OPTIONS: Array<{ id: Lang; label: string }> = [
    { id: 'en', label: 'English' },
    { id: 'zh-CN', label: '简体中文' },
    { id: 'zh-TW', label: '繁體中文' },
    { id: 'ja', label: '日本語' },
  ];
  const [currentVer, setCurrentVer] = React.useState('');
  const [updateInfo, setUpdateInfo] = React.useState<{ current: string; latest: string; available: boolean; download_url: string; release_url: string } | null>(null);
  const [updateStatus, setUpdateStatus] = React.useState<'idle' | 'checking' | 'applying' | 'restarting' | 'error'>('idle');
  const [updateError, setUpdateError] = React.useState('');
  React.useEffect(() => { fetch('/version').then(r => r.json()).then(d => setCurrentVer(d.version || '')).catch(() => {}); }, []);
  const checkForUpdates = async () => {
    setUpdateStatus('checking'); setUpdateError('');
    try {
      const r = await fetch('/update/check');
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const data = await r.json();
      setUpdateInfo(data);
      setUpdateStatus('idle');
    } catch (e: any) { setUpdateStatus('error'); setUpdateError(e?.message || 'Check failed'); }
  };
  const applyUpdate = async () => {
    setUpdateStatus('applying'); setUpdateError('');
    try {
      const r = await fetch('/update/apply', { method: 'POST' });
      if (!r.ok) {
        const text = await r.text().catch(() => '');
        let detail = text;
        try {
          const parsed = text ? JSON.parse(text) : {};
          detail = parsed.detail || parsed.message || text;
        } catch {}
        throw new Error(detail || `HTTP ${r.status}`);
      }
      setUpdateStatus('restarting'); setUpdateInfo(null);
      setTimeout(() => { window.location.reload(); }, 3000);
    } catch (e: any) { setUpdateStatus('error'); setUpdateError(e?.message || 'Update failed'); }
  };
  return <main className="main-panel settings-main"><header className="chat-header header-no-drawer"><div><h1>{t('settings.title')}</h1><span>API, language, theme, and follow-ups</span></div><HeaderThemeControl theme={props.theme} setTheme={props.setTheme} mode={'settings' as Mode} /></header><section className="settings-content"><label><span>{t('settings.apiBase')}</span><input value={props.apiBase} onChange={(e) => props.setApiBase(e.target.value)} /></label><label><span>{t('settings.apiKey')}</span><input value={props.apiKey} onChange={(e) => props.setApiKey(e.target.value)} type="password" /></label><label><span>{t('settings.language')}</span><select value={props.lang} onChange={(e) => { const next = e.target.value as Lang; props.setLang(next); setI18nLang(next); }}>{LANG_OPTIONS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label><label><span>{t('settings.theme')}</span><select value={props.theme} onChange={(e) => props.setTheme(e.target.value as Theme)}>{THEME_OPTIONS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label><label><span>{t('settings.followUpBehaviour')}</span><select value={props.followUpBehaviour} onChange={(e) => props.setFollowUpBehaviour(e.target.value as FollowUpBehaviour)}><option value="queue">Queue</option><option value="steer">Steer</option></select></label><label><span>{t('settings.composerEnterMode')}</span><select value={props.composerEnterMode} onChange={(e) => props.setComposerEnterMode(e.target.value as ComposerEnterMode)}><option value="enter-send">{t('settings.enterSend')}</option><option value="enter-newline">{t('settings.enterNewline')}</option></select></label><button className="mobile-icon-only" aria-label="Refresh connection" onClick={() => { props.loadModels(); props.loadSessions(); }}><RefreshCw /> <span className="btn-label">{t('settings.refreshConn')}</span></button><div style={{ marginTop: 24, borderTop: '1px solid var(--border)', paddingTop: 16 }}><h3 style={{ margin: '0 0 8px', fontSize: 16 }}>{t('settings.update')}</h3><p style={{ margin: '0 0 10px', color: 'var(--muted)', fontSize: 13 }}>{t('settings.version')}: <code>{currentVer || '...'}</code></p><div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}><button className="btn-wide" onClick={checkForUpdates} disabled={updateStatus === 'checking' || updateStatus === 'applying' || updateStatus === 'restarting'} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, padding: '8px 14px', border: '1px solid var(--border)', borderRadius: 12, background: 'var(--surface-2)', cursor: 'pointer', fontSize: 13 }}>{updateStatus === 'checking' ? t('settings.checkingUpdate') : <><RefreshCw size={15} /> {t('settings.checkUpdate')}</>}</button>{updateInfo && <span style={{ fontSize: 13 }}>{updateInfo.available ? <span style={{ color: 'var(--green)' }}>{t('settings.updateAvailable')}: {updateInfo.latest}</span> : <span style={{ color: 'var(--muted)' }}>{t('settings.upToDate')}</span>}</span>}{updateInfo?.available && updateInfo.release_url && <a href={updateInfo.release_url} target="_blank" rel="noopener noreferrer" style={{ fontSize: 13, color: 'var(--accent)' }}>{t('settings.viewRelease')}</a>}</div>{updateInfo?.available && <button className="btn-wide" onClick={applyUpdate} disabled={updateStatus === 'applying' || updateStatus === 'restarting'} style={{ marginTop: 10, display: 'inline-flex', alignItems: 'center', gap: 6, padding: '8px 14px', border: '1px solid var(--accent)', borderRadius: 12, background: 'var(--accent)', color: '#fff', cursor: 'pointer', fontSize: 13 }}>{updateStatus === 'applying' ? t('settings.installingUpdate') : updateStatus === 'restarting' ? t('settings.restartingUpdate') : <><Download size={15} /> {t('settings.installUpdate')}</>}</button>}{updateStatus === 'error' && <p style={{ margin: '8px 0 0', color: 'var(--danger)', fontSize: 13 }}>{updateError}</p>}</div></section></main>;
}

function ImageBrowser({ theme, setTheme, requestConfirm, initialImageFilename, writeHashRoute, mode, onNavigateToSettings }: { theme: Theme; setTheme: (v: Theme) => void; requestConfirm: (title: string, message: string, danger?: boolean) => Promise<boolean>; initialImageFilename?: string; writeHashRoute: (route: HashRoute) => void; mode?: Mode; onNavigateToSettings?: () => void }) {
  const MAX_PAGE_SIZE = 120;
  const MIN_PRELOAD_DISTANCE_PX = 1800;
  const [images, setImages] = useState<ImageEntry[]>([]);
  const [stats, setStats] = useState<ImageStats>({ total_images: 0, total_bytes: 0 });
  const [loading, setLoading] = useState(false);
  const [hasMore, setHasMore] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [selecting, setSelecting] = useState(false);
  const [modal, setModal] = useState<ImageEntry | null>(null);
  const [metadata, setMetadata] = useState<ImageMetadata | null>(null);
  const [metadataPlacement, setMetadataPlacement] = useState<'side' | 'bottom'>('side');
  const [modalMetadataOpen, setModalMetadataOpen] = useState(false);
  const [notice, setNotice] = useState('');
  const [imageMenu, setImageMenu] = useState<{ item: ImageEntry; x: number; y: number } | null>(null);
  const scrollRef = useRef<HTMLElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const modalImgRef = useRef<HTMLImageElement>(null);
  const modalSwipeStart = useRef<{ x: number; y: number; id: number; axis: 'x' | 'y' | null; dragged: boolean } | null>(null);
  const modalSwipeSuppressClick = useRef(false);
  const modalSwipeSuppressTimer = useRef<number | null>(null);
  const modalAnimating = useRef(false);
  const modalZoom = useRef({ scale: 1, x: 0, y: 0 });
  const modalPanStart = useRef<{ id: number; x: number; y: number; panX: number; panY: number; dragged: boolean } | null>(null);
  const modalPointers = useRef(new Map<number, { x: number; y: number }>());
  const modalPinchStart = useRef<{ distance: number; center: { x: number; y: number }; scale: number; x: number; y: number; imageCenter: { x: number; y: number } } | null>(null);
  const imagesRef = useRef<ImageEntry[]>([]);
  const loadingRef = useRef(false);
  const reloadQueuedRef = useRef(false);
  const refreshBusyRef = useRef(false);
  const hasMoreRef = useRef(true);
  const enc = (v: string) => encodeURIComponent(v);
  const selectedList = useMemo(() => images.filter((item) => selected.has(item.filename)).map((item) => item.filename), [images, selected]);
  const sortImages = (items: ImageEntry[]) => [...items].sort((a, b) => b.modified_at - a.modified_at || b.filename.localeCompare(a.filename));
  const formatImageBytes = (bytes?: number) => {
    if (bytes === undefined || !Number.isFinite(bytes) || bytes < 0) return '—';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
    const digits = unit === 0 ? 0 : value >= 10 ? 1 : 2;
    return `${value.toFixed(digits).replace(/\.0+$/, '')} ${units[unit]}`;
  };
  const downloadButtonLabel = (item: ImageEntry) => item.heic_status === 'missing' ? 'Generate HEIC' : 'Download';
  const triggerBrowserDownload = (url: string, filename: string) => {
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
  };
  const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));
  const getGridColumnCount = () => {
    const grid = gridRef.current;
    if (!grid) return 1;
    const style = getComputedStyle(grid);
    const cols = style.gridTemplateColumns.split(' ').filter(Boolean).length;
    if (cols > 0) return cols;
    const gap = parseFloat(style.columnGap || style.gap) || 18;
    const width = grid.clientWidth || window.innerWidth;
    return clamp(Math.floor((width + gap) / (260 + gap)), 1, 12);
  };
  const initialPageSizeForViewport = () => {
    const cols = getGridColumnCount();
    const toolbar = document.querySelector('.image-toolbar');
    const headerH = toolbar ? toolbar.getBoundingClientRect().height : 0;
    const viewportH = Math.max(320, window.innerHeight - headerH);
    const viewportW = Math.max(320, gridRef.current?.clientWidth || window.innerWidth);
    const colW = Math.max(120, viewportW / cols);
    const estimatedCardH = clamp(colW, window.innerWidth <= 760 ? 140 : 180, 420);
    const grid = gridRef.current;
    const gap = grid ? (parseFloat(getComputedStyle(grid).rowGap || getComputedStyle(grid).gap) || (window.innerWidth <= 760 ? 8 : 18)) : (window.innerWidth <= 760 ? 8 : 18);
    const rows = Math.max(1, Math.ceil(viewportH / (estimatedCardH + gap)) + 1);
    return clamp(cols * rows, cols * 2, MAX_PAGE_SIZE);
  };
  const lazyPageSizeForViewport = () => clamp(getGridColumnCount() * 2, 4, MAX_PAGE_SIZE);
  const pageSizeForViewport = (offset: number) => offset === 0 ? initialPageSizeForViewport() : lazyPageSizeForViewport();
  const preloadDistancePx = () => Math.max(MIN_PRELOAD_DISTANCE_PX, Math.round(window.innerHeight * 2.5));
  const MODAL_ANIM_MS = 230;
  const MODAL_MIN_ZOOM = 1;
  const MODAL_MAX_ZOOM = 6;
  const modalTravel = (axis: 'x' | 'y') => Math.round((axis === 'x' ? window.innerWidth : window.innerHeight) * 1.16);
  const modalTransform = (axis: 'x' | 'y', amount: number) => `translate3d(${axis === 'x' ? amount : 0}px, ${axis === 'y' ? amount : 0}px, 0)`;
  const modalZoomTransform = () => `translate3d(${modalZoom.current.x}px, ${modalZoom.current.y}px, 0) scale(${modalZoom.current.scale})`;
  const modalIsZoomed = () => modalZoom.current.scale > 1.01;
  const clampModalPan = () => {
    const img = modalImgRef.current;
    if (!img || !modalIsZoomed()) { modalZoom.current.x = 0; modalZoom.current.y = 0; return; }
    const extraX = Math.max(0, img.offsetWidth * (modalZoom.current.scale - 1) / 2 + 96);
    const extraY = Math.max(0, img.offsetHeight * (modalZoom.current.scale - 1) / 2 + 96);
    modalZoom.current.x = clamp(modalZoom.current.x, -extraX, extraX);
    modalZoom.current.y = clamp(modalZoom.current.y, -extraY, extraY);
  };
  const applyModalZoom = (transition = false) => {
    const img = modalImgRef.current;
    if (!img) return;
    clampModalPan();
    img.style.transition = transition ? 'transform 160ms cubic-bezier(.2,.8,.2,1)' : 'none';
    img.classList.toggle('zoomed', modalIsZoomed());
    img.style.transform = modalIsZoomed() ? modalZoomTransform() : '';
  };
  const resetModalZoom = () => {
    modalZoom.current = { scale: 1, x: 0, y: 0 };
    modalPanStart.current = null;
    modalPinchStart.current = null;
    modalPointers.current.clear();
    modalImgRef.current?.classList.remove('zoomed', 'panning');
  };
  const resetModalImageMotion = () => {
    const img = modalImgRef.current;
    resetModalZoom();
    if (!img) return;
    img.style.transition = '';
    img.style.transform = '';
    img.style.opacity = '';
  };
  const zoomModalAt = (clientX: number, clientY: number, nextScale: number) => {
    const img = modalImgRef.current;
    if (!img) return;
    const oldScale = modalZoom.current.scale;
    nextScale = clamp(nextScale, MODAL_MIN_ZOOM, MODAL_MAX_ZOOM);
    if (Math.abs(nextScale - oldScale) < 0.001) return;
    const rect = img.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const ratio = nextScale / oldScale;
    modalZoom.current.x -= (clientX - cx) * (ratio - 1);
    modalZoom.current.y -= (clientY - cy) * (ratio - 1);
    modalZoom.current.scale = nextScale;
    if (!modalIsZoomed()) modalZoom.current = { scale: 1, x: 0, y: 0 };
    applyModalZoom();
  };
  const suppressModalClickBriefly = () => {
    modalSwipeSuppressClick.current = true;
    if (modalSwipeSuppressTimer.current !== null) window.clearTimeout(modalSwipeSuppressTimer.current);
    modalSwipeSuppressTimer.current = window.setTimeout(() => { modalSwipeSuppressClick.current = false; modalSwipeSuppressTimer.current = null; }, 350);
  };
  const isMobilePreviewMode = () => window.matchMedia('(hover: none), (pointer: coarse)').matches || window.innerWidth <= 760;
  const modalPointerList = () => Array.from(modalPointers.current.values());
  const modalPointerDistance = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.hypot(a.x - b.x, a.y - b.y);
  const modalPointerCenter = (a: { x: number; y: number }, b: { x: number; y: number }) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
  const beginModalPan = (event: React.PointerEvent) => {
    modalPanStart.current = { id: event.pointerId, x: event.clientX, y: event.clientY, panX: modalZoom.current.x, panY: modalZoom.current.y, dragged: false };
    modalSwipeStart.current = null;
    modalImgRef.current?.classList.add('panning');
    if (modalImgRef.current) modalImgRef.current.style.transition = 'none';
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* ignore */ }
  };
  const beginModalPinch = () => {
    const pts = modalPointerList();
    const img = modalImgRef.current;
    if (pts.length < 2 || !img) return;
    const center = modalPointerCenter(pts[0], pts[1]);
    const rect = img.getBoundingClientRect();
    modalPinchStart.current = { distance: Math.max(1, modalPointerDistance(pts[0], pts[1])), center, scale: modalZoom.current.scale, x: modalZoom.current.x, y: modalZoom.current.y, imageCenter: { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 } };
    modalSwipeStart.current = null;
    modalPanStart.current = null;
    img.classList.add('panning');
    img.style.transition = 'none';
  };
  const snapModalImageBack = () => {
    const img = modalImgRef.current;
    if (!img) return;
    img.style.transition = `transform ${MODAL_ANIM_MS}ms cubic-bezier(.2,.8,.2,1), opacity ${MODAL_ANIM_MS}ms ease`;
    img.style.transform = modalTransform('x', 0);
    img.style.opacity = '1';
    window.setTimeout(resetModalImageMotion, MODAL_ANIM_MS + 40);
  };
  const animateModalClose = (axis: 'x' | 'y' = 'y', dir = 1) => {
    const img = modalImgRef.current;
    if (!img || modalAnimating.current) return;
    modalAnimating.current = true;
    img.style.transition = `transform ${MODAL_ANIM_MS}ms cubic-bezier(.2,.8,.2,1), opacity ${MODAL_ANIM_MS}ms ease`;
    img.style.transform = modalTransform(axis, dir * modalTravel(axis));
    img.style.opacity = '0';
    window.setTimeout(() => { modalAnimating.current = false; setModal(null); resetModalImageMotion(); }, MODAL_ANIM_MS + 40);
  };
  const updateImages = (next: ImageEntry[]) => { const sorted = sortImages(next); imagesRef.current = sorted; setImages(sorted); };
  const mergeImage = (entry: ImageEntry) => updateImages([entry, ...imagesRef.current.filter((x) => x.filename !== entry.filename)]);
  const openImageModal = (item: ImageEntry) => { setModal(item); setModalMetadataOpen(false); writeHashRoute({ mode: 'images', imageFilename: item.filename }); buildHashRoute({ mode: 'images', imageFilename: item.filename }); };
  const closeImageModal = () => { setModal(null); setModalMetadataOpen(false); writeHashRoute({ mode: 'images' }); };
  const removeImages = (names: string[]) => {
    const gone = new Set(names.filter(Boolean));
    updateImages(imagesRef.current.filter((x) => !gone.has(x.filename)));
    setSelected((old) => new Set(Array.from(old).filter((x) => !gone.has(x))));
    setModal((old) => old && gone.has(old.filename) ? null : old);
  };
  useEffect(() => {
    if (!initialImageFilename) return;
    fetch(`/image-api/images/${enc(initialImageFilename)}`, { cache: 'no-store' })
      .then((res) => res.ok ? res.json() : null)
      .then((entry: ImageEntry | null) => {
        if (!entry) return;
        mergeImage(entry);
        setModal(entry);
      })
      .catch(() => setNotice(`Image not found: ${initialImageFilename}`));
  }, [initialImageFilename]);
  const loadStats = useCallback(async () => {
    try { const res = await fetch('/image-api/stats', { cache: 'no-store' }); if (res.ok) setStats(await res.json()); }
    catch { /* ignore */ }
  }, []);
  const loadImages = useCallback(async (reset = false) => {
    if (loadingRef.current) { if (reset) reloadQueuedRef.current = true; return; }
    if (!reset && !hasMoreRef.current) return;
    loadingRef.current = true;
    setLoading(true);
    try {
      const offset = reset ? 0 : imagesRef.current.length;
      const pageSize = pageSizeForViewport(offset);
      const res = await fetch(`/image-api/images?offset=${offset}&limit=${pageSize}`, { cache: 'no-store' });
      if (!res.ok) throw new Error(await res.text());
      const chunk: ImageEntry[] = await res.json();
      const existing = reset ? [] : imagesRef.current;
      const existingNames = new Set(existing.map((item) => item.filename));
      updateImages(reset ? chunk : [...existing, ...chunk.filter((item) => !existingNames.has(item.filename))]);
      const more = chunk.length === pageSize;
      hasMoreRef.current = more;
      setHasMore(more);
      setNotice('');
      await loadStats();
    } catch (err: any) { setNotice(`Image API unavailable: ${err.message}`); }
    finally {
      loadingRef.current = false;
      setLoading(false);
      if (reloadQueuedRef.current) {
        reloadQueuedRef.current = false;
        loadImages(true);
      }
    }
  }, [loadStats]);
  const refreshIncremental = useCallback(async () => {
    if (refreshBusyRef.current) return;
    refreshBusyRef.current = true;
    setNotice('Refreshing…');
    try {
      const oldMap = new Map(imagesRef.current.map((item) => [item.filename, item]));
      const after = imagesRef.current.reduce((max, item) => Math.max(max, Number(item.modified_at || 0)), 0);
      const checkNames = imagesRef.current.filter((item) => !item.heic_url).slice(0, 240).map((item) => item.filename);
      const params = new URLSearchParams({ after: String(after), limit: String(MAX_PAGE_SIZE) });
      if (checkNames.length) params.set('check', checkNames.join(','));
      const res = await fetch(`/image-api/images/refresh?${params.toString()}`, { cache: 'no-store' });
      if (!res.ok) throw new Error(await res.text());
      const payload: { new_items?: ImageEntry[]; checked_items?: ImageEntry[] } = await res.json();
      let added = 0;
      let updated = 0;
      for (const item of [...payload.new_items || [], ...payload.checked_items || []]) {
        const old = oldMap.get(item.filename);
        if (old && old.heic_filename === item.heic_filename && old.image_url === item.image_url && old.png_url === item.png_url && old.heic_url === item.heic_url && old.modified_at === item.modified_at && old.size === item.size) continue;
        if (old) updated += 1;
        else added += 1;
        mergeImage(item);
      }
      if ((payload.new_items || []).length >= MAX_PAGE_SIZE) { hasMoreRef.current = true; setHasMore(true); }
      if (added || updated) setNotice(`Refresh complete: added ${added}, updated ${updated}`);
      else if (window.innerWidth > 760) setNotice('Refresh complete: no new images');
      await loadStats();
    } catch (err: any) { setNotice(`Refresh failed: ${err.message || err}`); }
    finally { refreshBusyRef.current = false; }
  }, [loadStats]);
  const refresh = refreshIncremental;
  useEffect(() => { loadImages(true); }, [loadImages]);
  useEffect(() => { imagesRef.current = images; }, [images]);
  useEffect(() => { hasMoreRef.current = hasMore; }, [hasMore]);
  useEffect(() => {
    const node = sentinelRef.current;
    if (!node) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) loadImages(false);
    }, { root: scrollRef.current, rootMargin: `${preloadDistancePx()}px` });
    observer.observe(node);
    return () => observer.disconnect();
  }, [loadImages]);
  const onImageScroll = (event: React.UIEvent<HTMLElement>) => {
    const el = event.currentTarget;
    if (el.scrollHeight - el.scrollTop - el.clientHeight < preloadDistancePx()) loadImages(false);
  };
  useEffect(() => {
    const es = new EventSource('/image-api/events');
    es.onopen = () => setNotice('');
    es.onmessage = (ev) => { try { const msg = JSON.parse(ev.data); if (msg.type === 'image' && msg.data) mergeImage(msg.data); if (msg.type === 'resync') refresh(); } catch { /* ignore */ } };
    es.addEventListener('delete', (ev) => { try { const msg = JSON.parse((ev as MessageEvent).data); removeImages(msg.filenames || [msg.filename]); } catch { /* ignore */ } });
    es.addEventListener('resync', refresh);
    es.onerror = () => setNotice('disconnected');
    return () => es.close();
  }, [refresh]);
  useEffect(() => {
    if (!modal) { setMetadata(null); setModalMetadataOpen(false); return; }
    setMetadata(null);
    fetch(`/image-api/images/${enc(modal.filename)}/metadata`, { cache: 'no-store' }).then((res) => res.ok ? res.json() : null).then(setMetadata).catch(() => setMetadata(null));
  }, [modal?.filename]);
  const modalImageUrl = (item: ImageEntry) => item.png_url || item.image_url;
  useEffect(() => { resetModalImageMotion(); }, [modal?.filename]);
  const modalIndex = modal ? images.findIndex((item) => item.filename === modal.filename) : -1;
  const navigateModal = useCallback(async (dir: -1 | 1) => {
    if (!modal) return;
    let idx = imagesRef.current.findIndex((item) => item.filename === modal.filename);
    if (dir > 0 && hasMoreRef.current && idx >= imagesRef.current.length - 2) await loadImages(false);
    idx = imagesRef.current.findIndex((item) => item.filename === modal.filename);
    const next = imagesRef.current[idx + dir];
    if (next) openImageModal(next);
  }, [loadImages, modal]);
  const onModalWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    if (!modal || modalAnimating.current) return;
    if ((event.target as HTMLElement).closest('.modalbar,.modal-meta,a,button')) return;
    event.preventDefault();
    event.stopPropagation();
    const factor = Math.exp(-event.deltaY * 0.0018);
    zoomModalAt(event.clientX, event.clientY, modalZoom.current.scale * factor);
  };
  const onModalImageClick = (event: React.MouseEvent<HTMLImageElement>) => {
    event.stopPropagation();
    if (modalIsZoomed()) return;
    if (!isMobilePreviewMode()) return;
    if (modalSwipeSuppressClick.current) { modalSwipeSuppressClick.current = false; return; }
    closeImageModal();
  };
  const onModalBackdropClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if (modalSwipeSuppressClick.current) { modalSwipeSuppressClick.current = false; return; }
    if (event.target === event.currentTarget) closeImageModal();
  };
  const onModalPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!modal || modalAnimating.current) return;
    if ((event.target as HTMLElement).closest('.modalbar,.modal-meta,a,button')) return;
    if (event.pointerType === 'mouse') {
      if (event.button !== 0 || !modalIsZoomed()) return;
      event.preventDefault();
      beginModalPan(event);
      return;
    }
    if (event.pointerType !== 'touch') return;
    modalPointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    modalSwipeSuppressClick.current = false;
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* ignore */ }
    if (modalPointers.current.size >= 2) { event.preventDefault(); beginModalPinch(); return; }
    if (modalIsZoomed()) { event.preventDefault(); beginModalPan(event); return; }
    modalSwipeStart.current = { x: event.clientX, y: event.clientY, id: event.pointerId, axis: null, dragged: false };
    if (modalImgRef.current) modalImgRef.current.style.transition = 'none';
  };
  const onModalPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    const img = modalImgRef.current;
    if (!img) return;
    if (event.pointerType === 'touch' && modalPointers.current.has(event.pointerId)) modalPointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (modalPinchStart.current && event.pointerType === 'touch' && modalPointers.current.size >= 2) {
      event.preventDefault();
      const pts = modalPointerList();
      const center = modalPointerCenter(pts[0], pts[1]);
      const distance = Math.max(1, modalPointerDistance(pts[0], pts[1]));
      const start = modalPinchStart.current;
      const scale = clamp(start.scale * distance / start.distance, MODAL_MIN_ZOOM, MODAL_MAX_ZOOM);
      const ratio = scale / start.scale;
      modalZoom.current.scale = scale;
      modalZoom.current.x = start.x + center.x - start.center.x - (start.center.x - start.imageCenter.x) * (ratio - 1);
      modalZoom.current.y = start.y + center.y - start.center.y - (start.center.y - start.imageCenter.y) * (ratio - 1);
      if (!modalIsZoomed()) modalZoom.current = { scale: 1, x: 0, y: 0 };
      applyModalZoom();
      return;
    }
    if (modalPanStart.current && event.pointerId === modalPanStart.current.id) {
      event.preventDefault();
      const dx = event.clientX - modalPanStart.current.x;
      const dy = event.clientY - modalPanStart.current.y;
      if (Math.hypot(dx, dy) > 3) modalPanStart.current.dragged = true;
      modalZoom.current.x = modalPanStart.current.panX + dx;
      modalZoom.current.y = modalPanStart.current.panY + dy;
      applyModalZoom();
      return;
    }
    if (event.pointerType !== 'touch' || !modalSwipeStart.current) return;
    const dx = event.clientX - modalSwipeStart.current.x;
    const dy = event.clientY - modalSwipeStart.current.y;
    const absX = Math.abs(dx);
    const absY = Math.abs(dy);
    if (Math.hypot(dx, dy) <= 8) return;
    event.preventDefault();
    modalSwipeStart.current.dragged = true;
    if (!modalSwipeStart.current.axis && (absX > 14 || absY > 14)) modalSwipeStart.current.axis = absX >= absY ? 'x' : 'y';
    const axis = modalSwipeStart.current.axis || (absX >= absY ? 'x' : 'y');
    const amount = axis === 'x' ? dx : dy;
    const travel = modalTravel(axis);
    img.style.transform = modalTransform(axis, amount);
    img.style.opacity = String(Math.max(0.72, 1 - Math.abs(amount) / travel * 0.35));
  };
  const finishModalPointer = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'touch') modalPointers.current.delete(event.pointerId);
    if (modalPinchStart.current && modalPointers.current.size < 2) {
      if (!modalIsZoomed()) resetModalZoom();
      modalPinchStart.current = null;
      modalImgRef.current?.classList.remove('panning');
      suppressModalClickBriefly();
      event.preventDefault();
      return;
    }
    if (modalPanStart.current && event.pointerId === modalPanStart.current.id) {
      const dragged = modalPanStart.current.dragged;
      modalPanStart.current = null;
      modalImgRef.current?.classList.remove('panning');
      if (dragged) { suppressModalClickBriefly(); event.preventDefault(); }
      return;
    }
    if (event.pointerType !== 'touch' || !modalSwipeStart.current) return;
    const start = modalSwipeStart.current;
    const dx = event.clientX - start.x;
    const dy = event.clientY - start.y;
    modalSwipeStart.current = null;
    const absX = Math.abs(dx);
    const absY = Math.abs(dy);
    const axis = start.axis || (absX >= absY ? 'x' : 'y');
    const amount = axis === 'x' ? dx : dy;
    if (start.dragged) { suppressModalClickBriefly(); event.preventDefault(); }
    if (Math.abs(amount) > 60 && Math.abs(amount) > (axis === 'x' ? absY : absX) * 1.2) {
      if (axis === 'y' && amount > 0) animateModalClose('y', 1);
      else navigateModal(amount < 0 ? 1 : -1);
    } else if (start.dragged) snapModalImageBack();
  };
  const cancelModalPointer = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'touch') modalPointers.current.delete(event.pointerId);
    modalPinchStart.current = null;
    modalPanStart.current = null;
    modalImgRef.current?.classList.remove('panning');
    if (modalSwipeStart.current?.dragged) snapModalImageBack();
    modalSwipeStart.current = null;
  };
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!modal) return;
      if (event.key === 'Escape') closeImageModal();
      if (event.key === 'ArrowLeft') { event.preventDefault(); navigateModal(-1); }
      if (event.key === 'ArrowRight') { event.preventDefault(); navigateModal(1); }
      if (event.key === 'Delete' || event.key === 'Backspace') { event.preventDefault(); deleteNames([modal.filename]); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [modal, navigateModal]);
  const adjustMetadataPlacement = () => {
    const img = modalImgRef.current;
    if (!img || !img.naturalWidth || !img.naturalHeight) return;
    setMetadataPlacement(img.naturalWidth >= img.naturalHeight ? 'bottom' : 'side');
  };
  useLayoutEffect(() => { adjustMetadataPlacement(); }, [modal?.filename]);
  useEffect(() => {
    if (!modal) return;
    const onResize = () => adjustMetadataPlacement();
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [modal]);
  const toggleSelect = (filename: string) => {
    if (!selecting) return;
    setSelected((old) => { const next = new Set(old); next.has(filename) ? next.delete(filename) : next.add(filename); return next; });
  };
  const toggleSelectionMode = () => {
    setSelecting((old) => {
      const next = !old;
      if (!next) setSelected(new Set());
      return next;
    });
  };
  const deleteNames = async (names: string[]) => {
    if (!names.length) return;
    if (!await requestConfirm('Delete images', `Delete ${names.length} image(s)? This cannot be undone.`, true)) return;
    const res = await fetch('/image-api/batch-delete', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ filenames: names }) });
    if (!res.ok) { setNotice(await res.text()); return; }
    removeImages(names); setSelecting(false); await loadStats();
  };
  const downloadSelectedFiles = async (names: string[]) => {
    if (!names.length) return;
    const selectedNameSet = new Set(names);
    const selectedItems = imagesRef.current.filter((item) => selectedNameSet.has(item.filename));
    for (const item of selectedItems) {
      triggerBrowserDownload(item.download_url || item.png_url, item.download_filename || item.filename);
      await new Promise((resolve) => window.setTimeout(resolve, 120));
    }
    setSelecting(false);
    setSelected(new Set());
  };
  const organizeTime = async () => {
    if (!selectedList.length) return;
    const res = await fetch('/image-api/batch-mtime', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ filenames: selectedList }) });
    if (!res.ok) { setNotice(await res.text()); return; }
    setSelecting(false); setSelected(new Set()); loadImages(true);
  };
  const generateHeic = async (item: ImageEntry): Promise<ImageEntry | null> => {
    setNotice(`Generating HEIC for ${item.filename}...`);
    const res = await fetch(`/image-api/images/${enc(item.filename)}/heic`, { method: 'POST' });
    if (!res.ok) { setNotice(await res.text()); return null; }
    const updated: ImageEntry = await res.json(); mergeImage(updated); setModal((old) => old?.filename === updated.filename ? updated : old); setNotice('HEIC generated.');
    return updated;
  };
  const downloadOne = async (item: ImageEntry) => {
    if (item.heic_status === 'missing') {
      const updated = await generateHeic(item);
      if (updated) triggerBrowserDownload(updated.download_url || updated.png_url, updated.download_filename || updated.filename);
      return;
    }
    triggerBrowserDownload(item.download_url || item.png_url, item.download_filename || item.filename);
  };
  const metadataEntries = (metadata?.png_text || []).filter((entry) => {
    const keyword = String(entry.keyword || '').toLowerCase();
    if (keyword !== 'description') return true;
    const prompt = metadata?.png_text?.find((item) => String(item.keyword || '').toLowerCase() === 'prompt');
    return !prompt || String(prompt.value || '') !== String(entry.value || '');
  }).slice(0, 8);
  return <main className="image-browser">
    <header className="image-toolbar header-no-drawer">
      <div><h1>Gallery</h1><span>{images.length}/{stats.total_images || '—'} loaded · {formatImageBytes(stats.total_bytes)}{notice ? ` · ${notice}` : ''}</span></div>
      <div className="image-actions">
        {selecting && selected.size > 0 && <><button className="mobile-icon-only" aria-label="Download selected" onClick={() => downloadSelectedFiles(selectedList)}><Download /> <span className="btn-label">Download selected</span></button><button className="mobile-icon-only" aria-label="Organize" onClick={organizeTime}><CalendarClock /> <span className="btn-label">Organize</span></button><button className="danger mobile-icon-only" aria-label="Delete selected" onClick={() => deleteNames(selectedList)}><Trash2 /> <span className="btn-label">Delete selected</span></button></>}
        <button className="icon-btn mobile-icon-only" aria-label={selecting ? 'Cancel selection' : 'Select images'} onClick={toggleSelectionMode}><CheckSquare /> <span className="btn-label">{selecting ? 'Cancel' : 'Select'}</span></button>
        <button className="icon-btn" aria-label="Refresh" onClick={refresh}><RefreshCw /></button>
        <HeaderThemeControl theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} />
      </div>
    </header>
    <section className="image-grid-wrap" ref={scrollRef} onScroll={onImageScroll}>
      <div className="image-grid" ref={gridRef}>{images.map((item) => <article className={`image-card ${selecting ? 'selecting' : ''} ${selected.has(item.filename) ? 'selected' : ''}`} key={item.filename} onClick={() => selecting ? toggleSelect(item.filename) : openImageModal(item)} onContextMenu={(event) => { event.preventDefault(); setImageMenu({ item, x: event.clientX, y: event.clientY }); }}>
        {selecting && <button type="button" aria-label={`Select ${item.filename}`} className={`image-checkbox ${selected.has(item.filename) ? 'checked' : ''}`} onClick={(event) => { event.stopPropagation(); toggleSelect(item.filename); }} />}
        <img loading="eager" decoding="async" src={item.image_url} alt={item.filename} onLoad={(event) => event.currentTarget.classList.add('loaded')} />
        <div className="image-overlay"><span className="image-name" title={item.filename}>{item.filename}</span><button className="mobile-icon-only" aria-label={downloadButtonLabel(item)} onClick={(event) => { event.stopPropagation(); downloadOne(item); }}>{item.heic_status === 'missing' ? <RefreshCw /> : <Download />}</button></div>
      </article>)}</div>
      {images.length === 0 && !loading && <div className="empty-state"><ImageIcon className="big-mark" /><h2>No images found</h2><p>The binary reads the configured image directory; default is HERMES_HOME/cache/images.</p></div>}
      <div ref={sentinelRef} className="image-sentinel">{loading ? 'Loading…' : hasMore ? 'Scroll to load more…' : 'End of images'}</div>
    </section>
    {imageMenu && <div className="image-context-menu" role="menu" style={{ left: imageMenu.x, top: imageMenu.y }} onContextMenu={(event) => event.preventDefault()} onClick={() => setImageMenu(null)}>
      <button type="button" role="menuitem" onClick={() => { triggerBrowserDownload(imageMenu.item.heic_url || imageMenu.item.png_url, imageMenu.item.heic_filename || imageMenu.item.filename); }}><Download /> Download HEIC</button>
      <button type="button" role="menuitem" onClick={() => { triggerBrowserDownload(imageMenu.item.png_url || imageMenu.item.image_url, imageMenu.item.filename); }}><Download /> Download PNG</button>
      <button type="button" role="menuitem" className="danger" onClick={() => { deleteNames([imageMenu.item.filename]); }}><Trash2 /> Delete</button>
    </div>}
    {modal && <div className={`image-modal ${metadataPlacement === 'bottom' ? 'metadata-bottom' : ''} ${modalMetadataOpen ? 'metadata-open' : ''}`} onClick={onModalBackdropClick} onWheel={onModalWheel} onPointerDown={onModalPointerDown} onPointerMove={onModalPointerMove} onPointerUp={finishModalPointer} onPointerCancel={cancelModalPointer}>
      <img ref={modalImgRef} className="image-modal-img" src={modalImageUrl(modal)} alt={modal.filename} onLoad={adjustMetadataPlacement} onClick={onModalImageClick} />
      <aside className={`modal-meta ${metadataPlacement === 'bottom' ? 'metadata-bottom' : ''}`} onClick={(event) => event.stopPropagation()}>
        <h2>Metadata</h2>
        {metadata?.dimensions && <p className="metadata-dim">Dimensions: {metadata.dimensions.width} × {metadata.dimensions.height}</p>}
        <section className="metadata-files-section"><span>Files</span><p>PNG {metadata?.png ? formatImageBytes(metadata.png.size) : formatImageBytes(modal.size)}</p><p>WebP {metadata?.webp ? formatImageBytes((metadata.webp as any).size) : '—'}</p><p>HEIC {metadata?.heic ? formatImageBytes((metadata.heic as any).size) : modal.heic_status}</p></section>
        <section className="metadata-png-section"><span>PNG metadata</span>{metadataEntries.length ? metadataEntries.map((entry) => <p key={entry.keyword}><b>{entry.keyword}</b><br />{entry.value}</p>) : <p>No PNG text chunk</p>}</section>
      </aside>
      <div className="modalbar" onClick={(event) => event.stopPropagation()}>
        <button className="mobile-icon-only" aria-label={downloadButtonLabel(modal)} onClick={() => downloadOne(modal)}>{modal.heic_status === 'missing' ? <RefreshCw /> : <Download />}</button>
        <button className={`mobile-icon-only modal-metadata-toggle ${modalMetadataOpen ? 'active' : ''}`} aria-label="Metadata" aria-expanded={modalMetadataOpen} onClick={() => setModalMetadataOpen((value) => !value)}><Info /></button>
        <button className="mobile-icon-only" aria-label="Previous" disabled={modalIndex <= 0} onClick={() => navigateModal(-1)}><ChevronLeft /></button>
        <button className="mobile-icon-only" aria-label="Next" disabled={modalIndex < 0 || (!hasMore && modalIndex >= images.length - 1)} onClick={() => navigateModal(1)}><ChevronRight /></button>
        <button className="mobile-icon-only" aria-label="Close" onClick={closeImageModal}><X /></button>
        <button className="danger mobile-icon-only" aria-label="Delete" onClick={() => deleteNames([modal.filename])}><Trash2 /></button>
      </div>
    </div>}
  </main>;
}
