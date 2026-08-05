import React, { lazy, Suspense, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ArrowUp, Bot, Brain, CalendarClock, CheckSquare, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, Circle as SelectionMark, CircleHelp, Code, Copy, Download, Eye, FileText, Folder, Globe, GripVertical, History, Home, Image as ImageIcon, Info, Layers, Lightbulb, LineChart, List, Maximize2, MessageSquare, Minimize2, Network, Palette, Paperclip, Pause, Pencil, Pin, PinOff, Play, PlayCircle as PlayMark, Plus, Puzzle, RefreshCw, Repeat, Save, Search, Send, Server, Settings, SlidersHorizontal, Square, Star, Terminal, Trash2, UserRound, Users, Video, Volume2, X } from 'lucide-react';
import { buildChatInputWithAttachments } from './attachmentPayload';
import { buildChatRequestBody } from './chatRequest';
import { buildCronPatch, cronEditableValues } from './cronEditor';
import { waitForCronRunOutput } from './cronRunOutput';
import { errorMessage, isAbortError } from './errorMessage';
import { formatFileSize } from './formatFileSize';
import { createStreamAnimator } from './streamAnimator';
import { currentModelDisplayOption, providerDisplayName } from './modelDisplay';
import { fallbackContextWindowForModel, latestMessageProviderForModel, resolvePreferredModelProvider as resolveModelProvider, selectModelOption } from './modelContext';
import { ChatTranscript, type ChatMessage, type ChatTurnMetrics } from './ChatTranscript';
import { sessionDisplayTitle, sessionHeaderTimes } from './sessionTime';
import { compactSessionPreview, latestSessionPreviewFromMessages } from './sessionPreview';
import { highlightSourceText } from './syntaxHighlight';
import { buildHashRoute, getCurrentHashRoute, pushHashRoute, type HashRoute } from './hashRoute';
import { formatHexDump } from './hexViewer';
import { areaPath, chartPoint, chartTooltipAlignment, chartTooltipLabel, chartTooltipPlacement, chartYAxisTicks, emptyTotals, finalizeTotals, fmtCompactAxisTick, fmtMoney, fmtPercent, fmtTokens, formatMetricValue, linePath, metricLabels, metricValue, modelDailyMetricValues, modelHourlyMetricValues, modelPeriodTotals, periodSlice, periodSources, stackedAreaPath, usageModelLabel, type UsageDay, type UsageHour, type UsageInsights, type UsageMetric, type UsageModel, type UsageSource, type UsageTotals } from './insights';
import { mergeTurnMetrics, normalizeChatMessage, readTurnMetrics } from './chatMessage';
import { normalizeMessageParts } from './messageReasoning';
import { visibleChatMessages } from './messageVisibility';
import { type TurnDetailMetadata } from './turnDetails';
import { shouldAutoLoadOlderForHiddenHistory, shouldLoadNewerFromScroll, shouldLoadOlderFromScroll, shouldLoadOlderFromWheel } from './chatHistoryScroll';
import { captureMessageScrollAnchor, restoreMessageScrollAnchor, type MessageScrollAnchor } from './chatScrollAnchor';
import { mergeMessageWindow } from './chatMessageWindow';
import { backfillOlderChunkToTurnBoundary, normalizeChatHistoryChunk, numericHistoryMessageId, type ChatHistoryPageRaw } from './chatHistoryPage';
import { computeNewMessageMarker } from './chatNewMessages';
import { chatLatestButtonVisible } from './chatLatest';
import { nextImageAfterRemoval, nextImageForPreload } from './imageBrowserNavigation';
import { isMarkdownPath, markdownText, chatMediaImagesFromMarkdown, type ChatMarkdownImage } from './markdown';

import { initLang, setLang as setI18nLang, getLang, t, tf, type Lang } from './i18n';
import { splitSidebarSessions } from './sessionListFilter';
import { MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH, readSidebarWidth, sidebarWidthFromKey, sidebarWidthFromPointer } from './sidebarWidth';
import { isTextEntryElement, visibleViewportHeight } from './viewport';
import { SubagentProgressCard } from './SubagentProgressCard';
import { subagentBeforeTimeForMessages, subagentPrecedingFallbackIds, subagentViewportIsLive } from './subagentProgress';

type Theme = 'hermes-light' | 'hermes-dark' | 'vscode-light-plus' | 'vscode-dark-plus' | 'monokai' | 'nord' | 'solarized-dark' | 'catppuccin-latte' | 'catppuccin-mocha' | 'nous' | 'gruvbox-material' | 'github-dark-dimmed';
type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'images' | 'workspace' | 'skills' | 'terminal' | 'settings';

const WebTerminal = lazy(() => import('./WebTerminal'));

type FollowUpBehaviour = 'queue' | 'steer';
type ComposerEnterMode = 'enter-send' | 'enter-newline';
type Session = { id: string; source?: string; title?: string; preview?: string; started_at?: number | string; ended_at?: number | string; last_active?: number | string; message_count?: number; input_tokens?: number; output_tokens?: number; model?: string; provider?: string };
function sessionWithPreservedMessageCount(next: Session, current?: Session | null): Session {
  if (!current || current.id !== next.id) return next;
  const merged: Session = { ...next };
  const currentCount = Number(current.message_count);
  const nextCount = Number(next.message_count);
  if (Number.isFinite(currentCount) && (!Number.isFinite(nextCount) || currentCount > nextCount)) merged.message_count = Math.trunc(currentCount);
  const currentTitle = String(current?.title || '').trim();
  const nextTitle = String(next.title || '').trim();
  if (!nextTitle && currentTitle) merged.title = current?.title;
  if (!String(next.provider || '').trim() && String(current.provider || '').trim()) merged.provider = current.provider;
  return merged;
}

type FollowUpQueueItem = { id: string; text: string; createdAt: number };
type ModelOption = { id: string; label: string; provider?: string; contextLength?: number };
type SessionModelOverride = { model: string; provider: string };
type Attachment = { id: string; name: string; kind: 'image' | 'text' | 'binary'; mime: string; size: number; dataUrl?: string; text?: string; uploadedPath?: string };
type SessionContextMenu = { session: Session; x: number; y: number } | null;

type SkillContextMenu = { skill: Skill; x: number; y: number } | null;
type WorkspaceEntry = { name: string; path: string; kind: 'file' | 'dir'; size?: number; modified?: string };
type WorkspacePreview = { path: string; content: string; kind: 'text' | 'image' | 'hex' | 'none'; url?: string; editRequest?: number; totalSize?: number; truncated?: boolean };
type Skill = { name: string; description?: string; version?: string; category?: string; enabled?: boolean };
type SkillFileContextMenu = { skill: Skill; entry: WorkspaceEntry; x: number; y: number } | null;
type WorkspaceContextMenu = { entry: WorkspaceEntry; x: number; y: number } | null;
type DialogState = { variant: 'prompt' | 'confirm'; title: string; message: string; value?: string; danger?: boolean; resolve: (value: any) => void } | null;
type Job = { job_id?: string; id?: string; name?: string; schedule?: string | { display?: string; expr?: string }; prompt?: string; script?: string | null; status?: string; paused?: boolean; enabled?: boolean; enabled_toolsets?: string[]; enabledToolsets?: string[]; model?: string | { model?: string; provider?: string }; provider?: string; provider_snapshot?: string; model_snapshot?: string; no_agent?: boolean; noAgent?: boolean; next_run?: string; last_run?: string; deliver?: string };
type CronOutput = { job_id?: string; timestamp?: string; filename?: string; content?: string; size_bytes?: number; truncated?: boolean };
type MemoryDoc = { memory: string; user: string };
type ImageEntry = { filename: string; heic_filename?: string | null; image_url: string; png_url: string; heic_url?: string | null; heic_status: 'available' | 'missing' | 'not_applicable' | string; download_filename: string; download_url: string; download_label: string; created_at: number; modified_at: number; size: number };
type ImageStats = { total_images: number; total_bytes: number };
type ImageFileMetadata = { size: number; [key: string]: unknown };
type ImageMetadata = { filename: string; dimensions?: { width: number; height: number } | null; png: ImageFileMetadata & { filename: string; url: string; modified_at: number }; webp?: ImageFileMetadata | null; heic?: ImageFileMetadata | null; heic_status: string; png_text: Array<{ keyword: string; value: string }> };
type ChatLightboxImage = ChatMarkdownImage & { key: string; messageId: string };
type RuntimeConfig = { api_url?: string; api_proxy_base?: string };

type MessagePage = ChatHistoryPageRaw & Required<Pick<ChatHistoryPageRaw, 'data' | 'has_older' | 'has_newer'>>;
type UserMessageNavItem = { id: string; role: 'user'; content: string; assistant_preview?: string; timestamp?: string | number; position: number; index: number; total: number };
type ContextWindowSnapshot = { sessionId: string; used: number; approximate?: boolean; compressed?: boolean };

const DEFAULT_API_BASE = '/hermes';
const SESSION_API_BASE = '/hermes';
const APP_BUILD_ID = 'details-i18n-v1';
const DRAFT_SESSION_ID = '__webui_draft_session__';
const FOLLOW_UP_BEHAVIOUR_KEY = 'followUpBehaviour';
const FOLLOW_UP_QUEUES_KEY = 'followUpQueues';
const COMPOSER_ENTER_MODE_KEY = 'composerEnterMode';
const CODE_WRAP_KEY = 'codeWrap';
const DESKTOP_COMPACT_MESSAGES_KEY = 'desktopCompactMessages';
const HIDE_CRON_SESSIONS_KEY = 'hideCronSessions';
const SIDEBAR_WIDTH_KEY = 'sidebarWidth';
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
  { id: 'gruvbox-material', label: 'Gruvbox Material' },
  { id: 'github-dark-dimmed', label: 'GitHub Dark Dimmed' },
  { id: 'nous', label: 'Nous' },
];
const DARK_THEMES = new Set<Theme>(['hermes-dark', 'vscode-dark-plus', 'monokai', 'nord', 'solarized-dark', 'catppuccin-mocha', 'nous', 'gruvbox-material', 'github-dark-dimmed']);
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
const RAW_MESSAGE_WINDOW = MESSAGE_WINDOW * 4;
const OTHER_PLATFORM_PENDING_ID = 'other-platform-pending';
const initialRoute = getCurrentHashRoute();

const uid = (prefix: string) => `${prefix}_${Date.now().toString(36)}_${Math.random().toString(16).slice(2)}`;
const clampNumber = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));
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
const jobSchedule = (schedule: Job['schedule']) => typeof schedule === 'string' ? schedule : (schedule?.display || schedule?.expr || t('cron.noSchedule'));
const jobState = (job: Job) => job.status || (job.paused || job.enabled === false ? 'paused' : 'active');
const jobStateLabel = (job: Job) => {
  const state = jobState(job);
  if (state === 'paused') return t('cron.paused');
  if (state === 'active') return t('cron.active');
  return state;
};
function cronEnabledToolsets(job?: Job | null): string[] {
  const raw = Array.isArray(job?.enabled_toolsets) ? job?.enabled_toolsets : Array.isArray(job?.enabledToolsets) ? job?.enabledToolsets : [];
  return Array.from(new Set((raw || []).map((item) => String(item || '').trim()).filter(Boolean)));
}
function cronPinnedModel(job?: Job | null) {
  if (job?.no_agent || job?.noAgent) return { nonAgent: true };
  const rawModel = job?.model;
  const model = typeof rawModel === 'object' ? String(rawModel?.model || '').trim() : String(rawModel || '').trim();
  const provider = (typeof rawModel === 'object' ? String(rawModel?.provider || '').trim() : '') || String(job?.provider || '').trim();
  if (!model && !provider) return null;
  return { model, provider, nonAgent: false };
}
const usageMetricLabel = (metric: UsageMetric) => t(`insights.metric.${metric}`);
const formatInsightCoverageStart = (timestamp: number) => new Date(timestamp * 1000).toLocaleString(getLang(), { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
function isHourlyBucket(bucket: UsageDay | UsageHour | undefined): bucket is UsageHour { return !!bucket && 'hour' in bucket; }
const navLabel = (mode: Mode) => t(`nav.${mode}`);
const modeSummary = (mode: Mode) => mode === 'memory' ? t('mode.memorySummary') : mode === 'insights' ? t('mode.insightsSummary') : mode === 'workspace' ? t('mode.workspaceSummary') : mode === 'terminal' ? t('mode.terminalSummary') : mode === 'settings' ? t('mode.settingsSummary') : mode === 'images' ? t('mode.imagesSummary') : t('mode.cronSummary');
const apiJoin = (base: string, path: string) => `${base.replace(/\/$/, '')}${path}`;
const numericId = numericHistoryMessageId;
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
function normalizeContent(value: unknown) {
  return normalizeMessageParts(value).content;
}
function normalizeMessage(raw: any, platformSource?: string): ChatMessage {
  return normalizeChatMessage(raw, uid('m'), platformSource);
}
function isLocalStreamAssistant(message: ChatMessage) {
  return message.role === 'assistant' && message.id.startsWith('assistant_');
}
function isLocalStreamTool(message: ChatMessage) {
  return message.role === 'tool' && message.id.startsWith('tool_');
}
function findCurrentTurnPersistedAssistantIndex(prev: ChatMessage[]) {
  let lastUserIndex = -1;
  for (let i = prev.length - 1; i >= 0; i -= 1) {
    if (prev[i].role === 'user') { lastUserIndex = i; break; }
  }
  return prev.findIndex((m, i) => i > lastUserIndex && m.role === 'assistant' && !isLocalStreamAssistant(m));
}
function findUnreconciledLocalAssistantIndex(prev: ChatMessage[]) {
  for (let i = prev.length - 1; i >= 0; i -= 1) {
    const msg = prev[i];
    if (isLocalStreamAssistant(msg)) return i;
    if (msg.role === 'user' || (msg.role === 'assistant' && !msg.pending && msg.id !== OTHER_PLATFORM_PENDING_ID)) break;
  }
  return -1;
}
function mergeWatchedMessage(prev: ChatMessage[], msg: ChatMessage): ChatMessage[] {
  if (msg.role === 'assistant' && isLocalStreamAssistant(msg)) {
    const currentTurnPersistedIdx = findCurrentTurnPersistedAssistantIndex(prev);
    if (currentTurnPersistedIdx >= 0) {
      const persistedId = prev[currentTurnPersistedIdx].id;
      return prev
        .filter((m) => m.id === persistedId || !isLocalStreamAssistant(m))
        .map((m) => m.id === persistedId ? { ...m, content: msg.content || m.content, reasoning: msg.reasoning || m.reasoning, pending: msg.pending } : m);
    }
  }
  if (prev.some((m) => m.id === msg.id)) return prev.map((m) => m.id === msg.id ? { ...m, ...msg } : m);
  // Match by content+role for user messages (server ID differs from local uid)
  if (msg.role === 'user') {
    const existing = prev.findIndex((m) => m.role === 'user' && m.content === msg.content);
    if (existing >= 0) return prev.map((m, i) => i === existing ? { ...m, ...msg, id: m.id } : m);
  }
  // Match pending assistant placeholder for assistant messages from watch.
  // Local browser-originated streams use a temporary assistant_* id; once the
  // server persists the same assistant message, the watch endpoint sends the
  // real id. Merge that persisted copy back into the local card instead of
  // appending a duplicate after the first streamed turn completes.
  if (msg.role === 'assistant') {
    const sameFinalIdx = isLocalStreamAssistant(msg)
      ? prev.findIndex((m) => m.role === 'assistant' && !isLocalStreamAssistant(m) && !m.pending && m.content === msg.content)
      : -1;
    if (sameFinalIdx >= 0) return prev;
    const pendingIdx = prev.findIndex((m) => m.pending && (m.id === OTHER_PLATFORM_PENDING_ID || isLocalStreamAssistant(m)));
    if (pendingIdx >= 0) return prev.map((m, i) => i === pendingIdx ? { ...m, ...msg, pending: false } : m);
    if (isLocalStreamAssistant(msg)) {
      const currentTurnPersistedIdx = findCurrentTurnPersistedAssistantIndex(prev);
      if (currentTurnPersistedIdx >= 0) {
        return prev.map((m, i) => i === currentTurnPersistedIdx ? { ...m, content: msg.content || m.content, reasoning: msg.reasoning || m.reasoning, pending: msg.pending } : m);
      }
    }
    const localStreamIdx = prev.findIndex((m) => isLocalStreamAssistant(m) && (m.content === msg.content || !isLocalStreamAssistant(msg)));
    if (localStreamIdx >= 0) {
      const localStreamId = prev[localStreamIdx].id;
      return prev
        .filter((m) => m.id === localStreamId || !isLocalStreamAssistant(m))
        .map((m) => m.id === localStreamId ? { ...m, ...msg, pending: false } : m);
    }
    const turnLocalStreamIdx = findUnreconciledLocalAssistantIndex(prev);
    if (turnLocalStreamIdx >= 0) return prev.map((m, i) => i === turnLocalStreamIdx ? { ...m, ...msg, pending: false } : m);
  }
  if (msg.role === 'tool') {
    const samePersistedToolIdx = isLocalStreamTool(msg)
      ? prev.findIndex((m) => m.role === 'tool' && !isLocalStreamTool(m) && (m.toolName || '') === (msg.toolName || ''))
      : -1;
    if (samePersistedToolIdx >= 0) return prev;
    if (!isLocalStreamTool(msg) && prev.some((m) => isLocalStreamTool(m) && (m.toolName || '') === (msg.toolName || ''))) {
      return prev.map((m) => isLocalStreamTool(m) && (m.toolName || '') === (msg.toolName || '') ? { ...m, ...msg, pending: false } : m);
    }
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
function readHideCronSessions() { return localStorage.getItem(HIDE_CRON_SESSIONS_KEY) === '1'; }
function readCodeWrap() { return localStorage.getItem(CODE_WRAP_KEY) !== '0'; }
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

function modelOptionKey(item: { id: string; provider?: string }) {
  return `${String(item.provider || '').trim()}\u0000${item.id}`;
}
function findModelOption(options: ModelOption[], modelId: string, provider = '') {
  return selectModelOption(options, realModelOrEmpty(modelId), provider);
}
function readContextLength(row: any): number | undefined {
  const raw = row?.context_length ?? row?.contextWindow ?? row?.context_window ?? row?.limit?.context;
  const value = Number(raw || 0);
  return Number.isFinite(value) && value > 0 ? value : undefined;
}
function flattenModelOptions(body: any): ModelOption[] {
  const seen = new Set<string>();
  const out: ModelOption[] = [];
  const push = (id: unknown, label?: unknown, provider?: unknown, contextLength?: number) => {
    const modelId = String(id || '').trim();
    const providerName = String(provider || '').trim();
    const key = `${providerName}\u0000${modelId}`;
    if (!modelId || modelId === 'hermes-agent' || seen.has(key)) return;
    seen.add(key);
    out.push(contextLength ? { id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined, contextLength } : { id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined });
  };
  if (Array.isArray(body?.providers)) {
    for (const provider of body.providers) {
      const providerId = provider?.slug || provider?.provider || provider?.id || provider?.name || '';
      const providerLabel = provider?.name || providerId;
      for (const modelId of provider?.models || []) push(modelId, `${providerLabel} · ${modelId}`, providerId, readContextLength(provider?.capabilities?.[modelId]));
    }
  }
  for (const modelRow of body?.data || []) {
    const contextLength = readContextLength(modelRow);
    push(modelRow?.id || modelRow, modelRow?.label || modelRow?.id, modelRow?.provider, contextLength);
  }
  return out;
}

function exactContextWindowTokens(messages: ChatMessage[]): number {
  return messages.reduce((sum, message) => sum + (message.tokenCount || 0), 0);
}
function roughTokenCount(text: string): number {
  const value = String(text || '').trim();
  if (!value) return 0;
  const cjk = value.match(/[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]/g)?.length || 0;
  const rest = Math.max(0, value.length - cjk);
  return Math.max(1, Math.ceil(cjk * 1.5 + rest / 4));
}
function estimateContextWindowTokens(messages: ChatMessage[], input: string, attachments: Attachment[]): number {
  const messageTokens = messages.reduce((sum, message) => {
    if (message.tokenCount !== undefined) return sum + message.tokenCount;
    return sum + roughTokenCount(message.content) + roughTokenCount(message.reasoning || '');
  }, 0);
  return messageTokens + estimateDraftContextTokens(input, attachments);
}
function estimateDraftContextTokens(input: string, attachments: Attachment[]): number {
  const inputTokens = roughTokenCount(input);
  const attachmentTokens = attachments.reduce((sum, attachment) => {
    if (attachment.text) return sum + roughTokenCount(attachment.text);
    if (attachment.kind === 'image') return sum + 85;
    return sum + Math.max(1, Math.ceil((attachment.size || 0) / 1024));
  }, 0);
  return inputTokens + attachmentTokens;
}
function contextWindowTokens(messages: ChatMessage[], input: string, attachments: Attachment[], hasUnloadedHistory: boolean, serverUsage?: { used?: number; approximate?: boolean }): { used: number; approximate: boolean } {
  if (serverUsage && typeof serverUsage.used === 'number' && Number.isFinite(serverUsage.used)) {
    const draftTokens = estimateDraftContextTokens(input, attachments);
    return { used: Math.max(0, serverUsage.used) + draftTokens, approximate: Boolean(serverUsage.approximate || draftTokens > 0) };
  }
  if (!input.trim() && !attachments.length && !hasUnloadedHistory && messages.length && messages.every((message) => !message.pending && message.tokenCount !== undefined)) {
    return { used: exactContextWindowTokens(messages), approximate: false };
  }
  return { used: estimateContextWindowTokens(messages, input, attachments), approximate: true };
}
function formatContextTokens(value: number): string {
  if (value >= 1000000) return `${(value / 1000000).toFixed(value % 1000000 === 0 ? 0 : 1)}M`;
  if (value >= 1000) return `${Math.round(value / 1000)}k`;
  return String(Math.max(0, Math.round(value)));
}
function ContextWindowMeter({ used, total, approximate = false }: { used?: number; total?: number; approximate?: boolean }) {
  if (!total) return null;
  const safeTotal = Math.max(1, total || 1);
  const hasUsed = typeof used === 'number' && Number.isFinite(used) && used >= 0;
  const safeUsed = hasUsed ? Math.max(0, used || 0) : undefined;
  const pct = safeUsed === undefined ? 0 : Math.min(100, Math.round(safeUsed / safeTotal * 100));
  const label = `${safeUsed === undefined ? '~' : `${approximate ? '~' : ''}${formatContextTokens(safeUsed)}`} / ${formatContextTokens(safeTotal)}`;
  const ariaLabel = approximate ? `Estimated context window ${label}` : safeUsed === undefined ? `Context window usage unavailable / ${formatContextTokens(safeTotal)}` : `Context window ${label}`;
  return <div className="context-window-meter" role="meter" aria-valuemin={0} aria-valuemax={safeTotal} aria-valuenow={safeUsed} title={ariaLabel} aria-label={ariaLabel}>
    <span className="context-window-track"><span className="context-window-fill" style={{ width: `${pct}%` }} /></span>
    <span className="context-window-label">{label}</span>
  </div>;
}

export default function App() {
  const [mode, setMode] = useState<Mode>(initialRoute.mode || 'chat');
  const [lang, setLangState] = useState<Lang>(initLang);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(initialRoute.mode === 'images' || initialRoute.mode === 'memory' || initialRoute.mode === 'insights' || initialRoute.mode === 'terminal' || initialRoute.mode === 'settings');
  const [sidebarWidth, setSidebarWidth] = useState(() => readSidebarWidth(localStorage.getItem(SIDEBAR_WIDTH_KEY)));
  const [sidebarResizing, setSidebarResizing] = useState(false);
  const sidebarWidthRef = useRef(sidebarWidth);
  const sidebarResizeRef = useRef<{ pointerId: number; left: number } | null>(null);
  const [workspaceCollapsed, setWorkspaceCollapsed] = useState(true);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [theme, setTheme] = useState<Theme>(() => normalizeTheme(localStorage.getItem('theme')));
  const [apiBase, setApiBase] = useState(() => localStorage.getItem('apiBase') || DEFAULT_API_BASE);
  const [apiServerUrl, setApiServerUrl] = useState('');
  const [apiKey, setApiKey] = useState(() => localStorage.getItem('apiKey') || '');
  const [models, setModels] = useState<ModelOption[]>([]);
  const [model, setModelState] = useState(readStoredModel);
  const [selectedModelProvider, setSelectedModelProvider] = useState('');
  const [sessionModelOverrides, setSessionModelOverrides] = useState<Record<string, SessionModelOverride>>({});
  const [followUpBehaviour, setFollowUpBehaviour] = useState<FollowUpBehaviour>(() => normalizeFollowUpBehaviour(localStorage.getItem(FOLLOW_UP_BEHAVIOUR_KEY)));
  const [composerEnterMode, setComposerEnterMode] = useState<ComposerEnterMode>(() => normalizeComposerEnterMode(localStorage.getItem(COMPOSER_ENTER_MODE_KEY)));
  const [codeWrap, setCodeWrap] = useState(readCodeWrap);
  const [followUpQueues, setFollowUpQueues] = useState<Record<string, FollowUpQueueItem[]>>(readFollowUpQueues);
  const [effort, setEffort] = useState<(typeof EFFORTS)[number]>(() => (localStorage.getItem('effort') as (typeof EFFORTS)[number]) || 'medium');
  const [sessions, setSessions] = useState<Session[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string>(initialRoute.mode === 'chat' ? initialRoute.sessionId || '' : '');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [userMessageNav, setUserMessageNav] = useState<UserMessageNavItem[]>([]);
  const [userNavLoading, setUserNavLoading] = useState(false);
  const [historyTotal, setHistoryTotal] = useState<number | null>(null);
  const [latestReadySessionId, setLatestReadySessionId] = useState('');
  const [contextWindowSnapshot, setContextWindowSnapshot] = useState<ContextWindowSnapshot | null>(null);
  const [showReasoning, setShowReasoning] = useState(true);
  const [desktopCompactMessages, setDesktopCompactMessages] = useState(() => localStorage.getItem(DESKTOP_COMPACT_MESSAGES_KEY) === '1');
  const [showToolCalls, setShowToolCalls] = useState(true);
  const [hasOlder, setHasOlder] = useState(false);
  const [hasNewer, setHasNewer] = useState(false);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [input, setInput] = useState('');
  const [composerCompact, setComposerCompact] = useState(false);
  const [filter, setFilter] = useState('');
  const [hideCronSessions, setHideCronSessions] = useState(readHideCronSessions);
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [toastMessage, setToastMessage] = useState('');
  const setStatus = useCallback((_value: string) => {}, []);
  const [busy, setBusy] = useState(false);
  const [streamingSessionId, setStreamingSessionId] = useState('');
  const streamingSessionIdRef = useRef('');
  const streamStatusProbeRef = useRef<{ sessionId: string; promise: Promise<boolean | null> } | null>(null);
  const [newMessageCount, setNewMessageCount] = useState(0);
  const [newMessageBoundaryId, setNewMessageBoundaryId] = useState('');
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
  const [skillMenu, setSkillMenu] = useState<SkillContextMenu>(null);
  const [skillFileMenu, setSkillFileMenu] = useState<SkillFileContextMenu>(null);
  const [workspaceMenu, setWorkspaceMenu] = useState<WorkspaceContextMenu>(null);
  const [cronJobs, setCronJobs] = useState<Job[]>([]);
  const [cronName, setCronName] = useState('');
  const [cronSchedule, setCronSchedule] = useState('0 9 * * *');
  const [cronPrompt, setCronPrompt] = useState('');
  const [cronScript, setCronScript] = useState('');
  const [cronDeliver, setCronDeliver] = useState('');
  const [cronOutput, setCronOutput] = useState<CronOutput | null>(null);
  const [cronOutputLoading, setCronOutputLoading] = useState(false);
  const [cronEditingId, setCronEditingId] = useState(initialRoute.mode === 'cron' ? initialRoute.jobId || '' : '');
  const [skillFilter, setSkillFilter] = useState('');
  const [skillRouteTarget, setSkillRouteTarget] = useState(initialRoute.mode === 'skills' ? initialRoute.skillName || '' : '');
  const [expandedSkillCats, setExpandedSkillCats] = useState<Set<string>>(new Set());
  const [workspaceRouteTarget, setWorkspaceRouteTarget] = useState<{ workspaceKind: 'file' | 'folder'; workspacePath: string; workspaceEdit?: boolean } | null>(initialRoute.mode === 'workspace' && initialRoute.workspaceKind ? { workspaceKind: initialRoute.workspaceKind, workspacePath: initialRoute.workspacePath || '' } : null);
  const [terminalCwd, setTerminalCwd] = useState(initialRoute.mode === 'terminal' ? initialRoute.cwd || '' : '');
  const [terminalMounted, setTerminalMounted] = useState(initialRoute.mode === 'terminal');
  const [usageInsights, setUsageInsights] = useState<UsageInsights | null>(null);
  const usageInsightsCacheRef = useRef<Partial<Record<1 | 7 | 30, UsageInsights>>>({});
  const [usageLoading, setUsageLoading] = useState(false);
  const [usageError, setUsageError] = useState('');
  const [usagePeriod, setUsagePeriod] = useState<1 | 7 | 30>(7);
  const [usageMetric, setUsageMetric] = useState<UsageMetric>('total_tokens');
  const [initialImageFilename, setInitialImageFilename] = useState(initialRoute.mode === 'images' ? initialRoute.imageFilename || '' : '');
  const chatScrollRef = useRef<HTMLElement | null>(null);
  const composerRef = useRef<HTMLElement | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const toastTimerRef = useRef<number | null>(null);
  const chatAbortRef = useRef<AbortController | null>(null);
  const messagesRef = useRef<ChatMessage[]>([]);
  const messageRequestRef = useRef(0);
  const contextWindowRequestRef = useRef(0);
  const userNavRequestRef = useRef(0);
  const loadingMessagesRef = useRef(false);
  const hasOlderRef = useRef(false);
  const hasNewerRef = useRef(false);
  const activeSessionIdRef = useRef(activeSessionId);
  const searchVersionRef = useRef(0);
  const modelsRef = useRef<ModelOption[]>(models);
  const modelRef = useRef(model);
  const providerRef = useRef(selectedModelProvider);
  const sessionModelOverridesRef = useRef(sessionModelOverrides);
  const showReasoningRef = useRef(showReasoning);
  const showToolCallsRef = useRef(showToolCalls);
  const newMessageBoundaryIdRef = useRef(newMessageBoundaryId);
  const renamedSessionTitlesRef = useRef<Record<string, string>>({});
  const routeEventHashRef = useRef('');
  const applyRenamedSessionTitleOverride = useCallback((session: Session) => {
    const titleOverride = renamedSessionTitlesRef.current[session.id];
    if (titleOverride && String(session.title || '').trim() !== titleOverride) return { ...session, title: titleOverride };
    if (titleOverride) delete renamedSessionTitlesRef.current[session.id];
    return session;
  }, []);
  useEffect(() => { messagesRef.current = messages; }, [messages]);
  useEffect(() => { modelsRef.current = models; }, [models]);
  useEffect(() => { modelRef.current = model; }, [model]);
  useEffect(() => { providerRef.current = selectedModelProvider; }, [selectedModelProvider]);
  useEffect(() => { sessionModelOverridesRef.current = sessionModelOverrides; }, [sessionModelOverrides]);
  useEffect(() => { showReasoningRef.current = showReasoning; }, [showReasoning]);
  useEffect(() => { showToolCallsRef.current = showToolCalls; }, [showToolCalls]);
  useEffect(() => { loadingMessagesRef.current = loadingMessages; }, [loadingMessages]);
  useEffect(() => { hasOlderRef.current = hasOlder; }, [hasOlder]);
  useEffect(() => { hasNewerRef.current = hasNewer; }, [hasNewer]);
  useEffect(() => { newMessageBoundaryIdRef.current = newMessageBoundaryId; }, [newMessageBoundaryId]);
  useEffect(() => { activeSessionIdRef.current = activeSessionId; }, [activeSessionId]);
  useEffect(() => () => { if (toastTimerRef.current !== null) window.clearTimeout(toastTimerRef.current); }, []);
  const showToast = useCallback((message: string) => {
    if (!message) return;
    setToastMessage(message);
    if (toastTimerRef.current !== null) window.clearTimeout(toastTimerRef.current);
    toastTimerRef.current = window.setTimeout(() => setToastMessage(''), 2600);
  }, []);
  const scrollLatestAfterRenderRef = useRef(false);
  const pendingHistoryScrollAnchorRef = useRef<MessageScrollAnchor | null>(null);
  const pendingJumpMessageIdRef = useRef('');
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
  const clearNewMessages = useCallback(() => {
    newMessageBoundaryIdRef.current = '';
    setNewMessageBoundaryId('');
    setNewMessageCount(0);
  }, []);

  const writeHashRoute = useCallback((route: HashRoute) => {
    pushHashRoute(window.history, window.location.hash, route);
  }, []);
  const switchActiveSession = useCallback((sessionId: string) => {
    activeSessionIdRef.current = sessionId;
    messageRequestRef.current += 1;
    messagesRef.current = [];
    hasOlderRef.current = false;
    hasNewerRef.current = false;
    pendingHistoryScrollAnchorRef.current = null;
    if (watchSourceRef.current) { watchSourceRef.current.close(); watchSourceRef.current = null; }
    setMessages([]);
    setUserMessageNav([]);
    setHasOlder(false);
    setHasNewer(false);
    clearNewMessages();
    setActiveSessionId(sessionId);
  }, [clearNewMessages]);
  const clearSelectedSkill = useCallback(() => {
    setSelectedSkillName('');
    setSkillRouteTarget('');
    setExpandedSkillPaths(new Set(['']));
    setSkillFileTree({});
    setSkillPreview({ path: '', content: '', kind: 'none' });
  }, []);
  const applyHashRoute = useCallback((route: HashRoute) => {
    setMode(route.mode);
    setSidebarCollapsed(route.mode === 'images' || route.mode === 'memory' || route.mode === 'insights' || route.mode === 'terminal' || route.mode === 'settings');
    if (route.mode !== 'chat' && route.mode !== 'cron') setMobileSidebarOpen(false);
    if (route.mode === 'chat' && route.sessionId) switchActiveSession(route.sessionId);
    if (route.mode === 'cron' && route.jobId) setCronEditingId(route.jobId);
    if (route.mode === 'skills' && route.skillName) setSkillRouteTarget(route.skillName);
    if (route.mode === 'skills' && !route.skillName) clearSelectedSkill();
    if (route.mode === 'images') setInitialImageFilename(route.imageFilename || '');
    if (route.mode === 'workspace' && route.workspaceKind) setWorkspaceRouteTarget({ workspaceKind: route.workspaceKind, workspacePath: route.workspacePath || '' });
    if (route.mode === 'terminal') { setTerminalMounted(true); setTerminalCwd(route.cwd || ''); }
  }, [clearSelectedSkill, switchActiveSession]);
  useEffect(() => {
    let clearRouteEventFrame = 0;
    const applyCurrentHashRoute = () => {
      const currentHash = window.location.hash;
      if (routeEventHashRef.current === currentHash) return;
      routeEventHashRef.current = currentHash;
      applyHashRoute(getCurrentHashRoute());
      window.cancelAnimationFrame(clearRouteEventFrame);
      clearRouteEventFrame = window.requestAnimationFrame(() => {
        if (routeEventHashRef.current === currentHash) routeEventHashRef.current = '';
      });
    };
    window.addEventListener('popstate', applyCurrentHashRoute);
    window.addEventListener('hashchange', applyCurrentHashRoute);
    applyCurrentHashRoute();
    return () => {
      window.cancelAnimationFrame(clearRouteEventFrame);
      window.removeEventListener('popstate', applyCurrentHashRoute);
      window.removeEventListener('hashchange', applyCurrentHashRoute);
    };
  }, [applyHashRoute]);

  useLayoutEffect(() => {
    const stableHeight = { current: visibleViewportHeight(window) };
    const syncViewportHeight = () => {
      const nextHeight = visibleViewportHeight(window);
      if (isTextEntryElement(document.activeElement) && nextHeight < stableHeight.current) return;
      stableHeight.current = nextHeight;
      document.documentElement.style.setProperty('--app-viewport-height', `${nextHeight}px`);
    };
    const syncAfterFocusChange = () => window.requestAnimationFrame(syncViewportHeight);
    syncViewportHeight();
    window.addEventListener('resize', syncViewportHeight);
    window.visualViewport?.addEventListener('resize', syncViewportHeight);
    window.visualViewport?.addEventListener('scroll', syncViewportHeight);
    document.addEventListener('focusout', syncAfterFocusChange);
    return () => {
      window.removeEventListener('resize', syncViewportHeight);
      window.visualViewport?.removeEventListener('resize', syncViewportHeight);
      window.visualViewport?.removeEventListener('scroll', syncViewportHeight);
      document.removeEventListener('focusout', syncAfterFocusChange);
      document.documentElement.style.removeProperty('--app-viewport-height');
    };
  }, []);
  useEffect(() => { document.documentElement.dataset.yahuBuild = APP_BUILD_ID; }, []);
  useEffect(() => { document.documentElement.classList.toggle('dark', isDarkTheme(theme)); document.documentElement.dataset.theme = theme; delete document.documentElement.dataset.skin; localStorage.setItem('theme', theme); localStorage.removeItem('skin'); }, [theme]);
  useEffect(() => { fetch('/runtime-config').then((res) => res.ok ? res.json() : null).then((config: RuntimeConfig | null) => { if (config?.api_url) setApiServerUrl(config.api_url); if (config?.api_proxy_base && !localStorage.getItem('apiBase')) setApiBase(config.api_proxy_base); }).catch(() => {}); }, []);
  useEffect(() => localStorage.setItem('apiBase', apiBase), [apiBase]);
  useEffect(() => localStorage.setItem('apiKey', apiKey), [apiKey]);
  useEffect(() => { const next = realModelOrEmpty(model); if (next) localStorage.setItem('model', next); }, [model]);
  useEffect(() => localStorage.setItem(FOLLOW_UP_BEHAVIOUR_KEY, followUpBehaviour), [followUpBehaviour]);
  useEffect(() => localStorage.setItem(COMPOSER_ENTER_MODE_KEY, composerEnterMode), [composerEnterMode]);
  useEffect(() => localStorage.setItem(CODE_WRAP_KEY, codeWrap ? '1' : '0'), [codeWrap]);
  useEffect(() => localStorage.setItem('effort', effort), [effort]);
  useEffect(() => localStorage.setItem(DESKTOP_COMPACT_MESSAGES_KEY, desktopCompactMessages ? '1' : '0'), [desktopCompactMessages]);
  useEffect(() => localStorage.setItem(HIDE_CRON_SESSIONS_KEY, hideCronSessions ? '1' : '0'), [hideCronSessions]);
  useEffect(() => localStorage.setItem('pinnedSessions', JSON.stringify(Array.from(pinnedIds))), [pinnedIds]);

  const activeSession = sessions.find((s) => s.id === activeSessionId) || (activeSessionDetail?.id === activeSessionId ? activeSessionDetail : undefined);
  useEffect(() => {
    if (streamingSessionId !== activeSessionId) return;
    const activePreview = latestSessionPreviewFromMessages(messages);
    if (!activeSessionId || activeSessionId === DRAFT_SESSION_ID || !activePreview) return;
    setSessions((old) => old.map((session) => session.id === activeSessionId && session.preview !== activePreview ? { ...session, preview: activePreview } : session));
    setActiveSessionDetail((old) => old?.id === activeSessionId && old.preview !== activePreview ? { ...old, preview: activePreview } : old);
  }, [activeSessionId, messages, streamingSessionId]);

  useEffect(() => {
    const override = activeSessionId ? sessionModelOverrides[activeSessionId] : undefined;
    if (override?.model) {
      modelRef.current = override.model;
      providerRef.current = override.provider;
      setModelState((current) => override.model !== current ? override.model : current);
      setSelectedModelProvider((current) => override.provider !== current ? override.provider : current);
      return;
    }
    const activeModel = realModelOrEmpty(activeSession?.model);
    const activeProvider = String(activeSession?.provider || '').trim();
    if (activeModel) {
      modelRef.current = activeModel;
      providerRef.current = activeProvider;
      setModelState((current) => activeModel !== current ? activeModel : current);
      setSelectedModelProvider((current) => activeProvider !== current ? activeProvider : current);
    }
  }, [activeSessionId, activeSession?.model, activeSession?.provider, sessionModelOverrides]);

  const filteredSessions = useMemo(() => {
    return splitSidebarSessions(sessions, pinnedIds);
  }, [sessions, pinnedIds]);

  const loadModels = useCallback(async () => {
    try {
      const res = await fetch('/models-cache');
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      const list = flattenModelOptions(body);
      const current = realModelOrEmpty(activeSession?.model) || realModelOrEmpty(model);
      const currentProvider = String(activeSession?.provider || providerRef.current || selectedModelProvider || '').trim();
      if (list.length) {
        const provider = current ? resolveModelProvider(list, current, currentProvider) : '';
        setModels(list);
        if (!current) {
          modelRef.current = list[0].id;
          providerRef.current = String(list[0].provider || '').trim();
          setModelState(list[0].id);
          setSelectedModelProvider(String(list[0].provider || '').trim());
        } else {
          providerRef.current = provider;
          setSelectedModelProvider(provider);
        }
      }
      setStatus(t('status.modelsLoaded'));
    } catch (err) { setStatus(tf('status.modelsUnavailable', errorMessage(err))); }
  }, [activeSession?.model, activeSession?.provider, model, selectedModelProvider, setStatus]);

  const loadUsageInsights = useCallback(async (period: 1 | 7 | 30 = usagePeriod, force = false) => {
    const cached = usageInsightsCacheRef.current[period];
    if (cached && !force) { setUsageInsights(cached); setUsageError(''); return; }
    setUsageLoading(true);
    setUsageError('');
    try {
      const timezoneOffset = new Date().getTimezoneOffset();
      const usageParams = new URLSearchParams({ period: String(period), tz_offset: String(timezoneOffset) });
      if (force) usageParams.set('refresh', 'true');
      const usageRes = await fetch(`/insights/usage?${usageParams.toString()}`);
      if (!usageRes.ok) throw new Error(await usageRes.text());
      const nextInsights = await usageRes.json();
      usageInsightsCacheRef.current[period] = nextInsights;
      setUsageInsights(nextInsights);
    } catch (err) {
      setUsageError(errorMessage(err, t('insights.unavailable')));
    } finally {
      setUsageLoading(false);
    }
  }, [usagePeriod]);

  const loadSessions = useCallback(async (query = filter) => {
    const version = ++searchVersionRef.current;
    try {
      const params = new URLSearchParams({ limit: '80', _: String(Date.now()) });
      if (query.trim()) params.set('q', query.trim());
      if (hideCronSessions) params.set('hide_cron_cli', 'true');
      if (pinnedIds.size) params.set('pinned_ids', Array.from(pinnedIds).join(','));
      const res = await fetch(`/sessions/search?${params}`, { headers: headers(false), cache: 'no-store' });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      if (version !== searchVersionRef.current) return;
      const list: Session[] = body.data || [];
      setSessions((old) => list.map((rawSession) => {
        const session = applyRenamedSessionTitleOverride(rawSession);
        return sessionWithPreservedMessageCount(session, old.find((existing) => existing.id === session.id));
      }));
      if (!activeSessionIdRef.current && list.length) switchActiveSession(list[0].id);
      setStatus(t('chat.connected'));
    } catch (err) { setStatus(tf('status.sessionsUnavailable', errorMessage(err))); }
  }, [filter, hideCronSessions, pinnedIds, headers, switchActiveSession, applyRenamedSessionTitleOverride]);

  const loadSessionDetail = useCallback(async (sessionId: string) => {
    if (!sessionId) return;
    if (sessionId === DRAFT_SESSION_ID) return;
    try {
      const res = await fetch(apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(sessionId)}`), { headers: headers(false) });
      if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
      const body = await res.json();
      const detail = applyRenamedSessionTitleOverride((body.data || body.session || body) as Session);
      setActiveSessionDetail((old) => sessionWithPreservedMessageCount(detail, old));
      setSessions((old) => old.some((s) => s.id === detail.id) ? old.map((s) => s.id === detail.id ? { ...s, ...sessionWithPreservedMessageCount(detail, s) } : s) : [detail, ...old]);
    } catch (err) { setStatus(tf('status.sessionDetailUnavailable', errorMessage(err))); }
  }, [apiBase, headers, applyRenamedSessionTitleOverride]);

  const updateSessionMessageCount = useCallback((sessionId: string, total: unknown) => {
    const parsed = Number(total);
    if (!sessionId || !Number.isFinite(parsed) || parsed < 0) return;
    const message_count = Math.trunc(parsed);
    setActiveSessionDetail((old) => old?.id === sessionId ? { ...old, message_count } : old);
    setSessions((old) => old.map((session) => session.id === sessionId ? { ...session, message_count } : session));
  }, []);

  const updateSessionBoundaryTimes = useCallback((sessionId: string, page: Pick<ChatHistoryPageRaw, 'started_at' | 'last_active'>) => {
    if (!sessionId || sessionId === DRAFT_SESSION_ID) return;
    const patch: Partial<Session> = {};
    if (page.started_at !== undefined) patch.started_at = page.started_at;
    if (page.last_active !== undefined) patch.last_active = page.last_active;
    if (!Object.keys(patch).length) return;
    setActiveSessionDetail((old) => old?.id === sessionId ? { ...old, ...patch } : old);
    setSessions((old) => old.map((session) => session.id === sessionId ? { ...session, ...patch } : session));
  }, []);

  const loadContextWindowSnapshot = useCallback(async (sessionId: string) => {
    const req = ++contextWindowRequestRef.current;
    if (!sessionId || sessionId === DRAFT_SESSION_ID) {
      setContextWindowSnapshot(null);
      return;
    }
    try {
      const res = await fetch(`/chat/context-window/${encodeURIComponent(sessionId)}`);
      if (!res.ok) throw new Error(await res.text());
      const body = await res.json();
      if (req !== contextWindowRequestRef.current || activeSessionIdRef.current !== sessionId) return;
      const used = Number(body.used);
      setContextWindowSnapshot(Number.isFinite(used) ? { sessionId, used, approximate: Boolean(body.approximate), compressed: Boolean(body.compressed) } : null);
    } catch { if (req === contextWindowRequestRef.current) setContextWindowSnapshot(null); }
  }, []);

  const changeSessionModel = useCallback((nextModel: string, option?: ModelOption) => {
    const resolvedModel = realModelOrEmpty(nextModel);
    if (!resolvedModel) return;
    const provider = resolveModelProvider(modelsRef.current, resolvedModel, option?.provider || providerRef.current);
    modelRef.current = resolvedModel;
    providerRef.current = provider;
    setModelState(resolvedModel);
    setSelectedModelProvider(provider);
    if (activeSessionId) {
      const nextOverrides = { ...sessionModelOverridesRef.current, [activeSessionId]: { model: resolvedModel, provider } };
      sessionModelOverridesRef.current = nextOverrides;
      setSessionModelOverrides(nextOverrides);
    }
    if (activeSessionId === DRAFT_SESSION_ID) setActiveSessionDetail((old) => old ? { ...old, model: resolvedModel, provider } : old);
    if (activeSessionId && activeSessionId !== DRAFT_SESSION_ID) {
      setActiveSessionDetail((old) => old?.id === activeSessionId ? { ...old, model: resolvedModel, provider } : old);
      setSessions((old) => old.map((s) => s.id === activeSessionId ? { ...s, model: resolvedModel, provider } : s));
    }
    setStatus(t('status.sessionModelSelected'));
  }, [activeSessionId]);

  const refreshSessionTitleOnce = useCallback(async (sessionId: string) => {
    if (!sessionId || sessionId === DRAFT_SESSION_ID) return;
    if (titleRefreshDoneRef.current.has(sessionId)) return;
    titleRefreshDoneRef.current.add(sessionId);
    await new Promise((resolve) => window.setTimeout(resolve, 450));
    await loadSessionDetail(sessionId);
  }, [loadSessionDetail]);

  const reconcileEffectiveSession = useCallback(async (previousSessionId: string, effectiveSessionId: string, createdSession: Session | null) => {
    if (!effectiveSessionId || previousSessionId === effectiveSessionId) return previousSessionId;
    skipNextHistoryLoadRef.current = effectiveSessionId;
    const effectiveSession = createdSession ? { ...createdSession, id: effectiveSessionId } : { id: effectiveSessionId };
    activeSessionIdRef.current = effectiveSessionId;
    setActiveSessionId(effectiveSessionId);
    setActiveSessionDetail(effectiveSession);
    setSessions((old) => [effectiveSession, ...old.filter((s) => s.id !== previousSessionId && s.id !== effectiveSessionId)]);
    writeHashRoute({ mode: 'chat', sessionId: effectiveSessionId });
    if (createdSession && previousSessionId !== effectiveSessionId) {
      await fetch(apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(previousSessionId)}`), { method: 'DELETE', headers: headers(false) }).catch(() => null);
    }
    await loadSessionDetail(effectiveSessionId);
    return effectiveSessionId;
  }, [apiBase, headers, loadSessionDetail, writeHashRoute]);

  const createSession = useCallback(async () => {
    const sessionModel = realModelOrEmpty(modelRef.current) || models[0]?.id || '';
    const sessionProvider = resolveModelProvider(modelsRef.current, sessionModel, providerRef.current);
    const sessionBody = sessionProvider ? { model: sessionModel, provider: sessionProvider } : { model: sessionModel };
    const res = await fetch(apiJoin(SESSION_API_BASE, '/api/sessions'), { method: 'POST', headers: headers(), body: JSON.stringify(sessionBody) });
    if (!res.ok) throw new Error(await res.text());
    const body = await res.json();
    return (body.session || body.data || body) as Session;
  }, [apiBase, headers, models]);

  const startDraftSession = useCallback(() => {
    const sessionModel = realModelOrEmpty(model) || models[0]?.id || '';
    messageRequestRef.current += 1;
    activeSessionIdRef.current = DRAFT_SESSION_ID;
    messagesRef.current = [];
    hasOlderRef.current = false;
    hasNewerRef.current = false;
    pendingHistoryScrollAnchorRef.current = null;
    if (watchSourceRef.current) { watchSourceRef.current.close(); watchSourceRef.current = null; }
    setActiveSessionId(DRAFT_SESSION_ID);
    setActiveSessionDetail({ id: DRAFT_SESSION_ID, model: sessionModel, provider: selectedModelProvider });
    setMessages([]);
    setUserMessageNav([]);
    setHasOlder(false);
    setHasNewer(false);
    setInput('');
    setAttachments([]);
    setStatus(t('status.draftConversation'));
    setSessionMenu(null);
    writeHashRoute({ mode: 'chat' });
  }, [model, models, selectedModelProvider, writeHashRoute]);

  const loadMessageWindow = useCallback(async (sessionId: string, direction: 'latest' | 'older' | 'newer' = 'latest') => {
    if (sessionId === DRAFT_SESSION_ID) return;
    if (!sessionId) return;
    if (loadingMessagesRef.current && direction !== 'latest') return;
    const scroller = chatScrollRef.current;
    if (direction === 'older') pendingHistoryScrollAnchorRef.current = captureMessageScrollAnchor(scroller);
    const req = ++messageRequestRef.current;
    loadingMessagesRef.current = true;
    setLoadingMessages(true);
    try {
      const params = new URLSearchParams({ limit: String(MESSAGE_PAGE), view: 'skeleton' });
      if (direction === 'latest') params.set('view', 'latest');
      if (direction === 'older') {
        const before = numericId(messagesRef.current[0]?.id);
        if (!before) { pendingHistoryScrollAnchorRef.current = null; return; }
        params.set('before', before);
      }
      if (direction === 'newer') {
        const after = numericId(messagesRef.current[messagesRef.current.length - 1]?.id);
        if (!after) return;
        params.set('after', after);
      }
      const fetchMessagePage = async (query: URLSearchParams): Promise<MessagePage> => {
        const response = await fetch(`/chat/messages/${encodeURIComponent(sessionId)}?${query}`);
        if (!response.ok) throw new Error(await response.text());
        return response.json();
      };
      const normalizePageChunk = (items: ChatHistoryPageRaw['data']) => normalizeChatHistoryChunk<ChatMessage>(items, (raw) => normalizeMessage(raw, activeSession?.source));
      const page = await fetchMessagePage(params);
      let chunk = normalizePageChunk(page.data || []);
      let pageHasOlder = Boolean(page.has_older);
      let pageHasNewer = Boolean(page.has_newer);
      let boundaryPage: ChatHistoryPageRaw = page;
      if (direction === 'older') {
        ({ chunk, pageHasOlder, pageHasNewer, boundaryPage } = await backfillOlderChunkToTurnBoundary({
          firstPage: page,
          firstChunk: chunk,
          fetchBefore: async (before, limit) => fetchMessagePage(new URLSearchParams({ limit: String(limit), before, view: 'skeleton' })),
          normalizeChunk: normalizePageChunk,
          numericId,
          pageLimit: MESSAGE_PAGE,
          rawWindowLimit: RAW_MESSAGE_WINDOW,
        }));
      }
      if (req !== messageRequestRef.current || activeSessionIdRef.current !== sessionId) return;
      updateSessionMessageCount(sessionId, page.total);
      updateSessionBoundaryTimes(sessionId, direction === 'older' ? boundaryPage : page);
      const merged = mergeMessageWindow<ChatMessage>({
        current: messagesRef.current,
        chunk,
        direction,
        limit: RAW_MESSAGE_WINDOW,
        hasOlder: hasOlderRef.current,
        hasNewer: hasNewerRef.current,
        pageHasOlder,
        pageHasNewer,
      });
      messagesRef.current = merged.messages;
      setMessages(merged.messages);
      if (direction === 'latest') setLatestReadySessionId(sessionId);
      hasOlderRef.current = merged.hasOlder;
      hasNewerRef.current = merged.hasNewer;
      setHasOlder(merged.hasOlder);
      setHasNewer(merged.hasNewer);
      if (direction === 'latest') scrollLatestAfterRenderRef.current = true;
    } catch (err) { setStatus(tf('status.messagesUnavailable', errorMessage(err))); }
    finally {
      if (req === messageRequestRef.current) {
        loadingMessagesRef.current = false;
        setLoadingMessages(false);
      }
    }
  }, [activeSession?.source, updateSessionBoundaryTimes, updateSessionMessageCount]);

  const loadUserMessageNav = useCallback(async (sessionId: string) => {
    const req = ++userNavRequestRef.current;
    if (!sessionId || sessionId === DRAFT_SESSION_ID) {
      setUserMessageNav([]);
      setHistoryTotal(null);
      setUserNavLoading(false);
      return;
    }
    try {
      const res = await fetch(`/chat/user-nav/${encodeURIComponent(sessionId)}`);
      if (!res.ok) throw new Error(await res.text());
      const body = await res.json();
      if (req !== userNavRequestRef.current || activeSessionIdRef.current !== sessionId) return;
      const total = Number(body.total);
      if (Number.isFinite(total) && total >= 0) setHistoryTotal(Math.trunc(total));
      updateSessionMessageCount(sessionId, body.total);
      setUserMessageNav(Array.isArray(body.data) ? body.data : []);
    } catch {
      if (req === userNavRequestRef.current && activeSessionIdRef.current === sessionId) setUserMessageNav([]);
    } finally {
      if (req === userNavRequestRef.current && activeSessionIdRef.current === sessionId) setUserNavLoading(false);
    }
  }, [updateSessionMessageCount]);

  const jumpToMessage = useCallback(async (sessionId: string, messageId: string) => {
    if (!sessionId || !messageId || sessionId === DRAFT_SESSION_ID) return;
    const targetId = String(messageId);
    const existing = document.querySelector(`[data-message-id="${CSS.escape(targetId)}"]`);
    if (existing) {
      existing.scrollIntoView({ block: 'center' });
      return;
    }
    const around = numericId(messageId);
    if (!around) return;
    const req = ++messageRequestRef.current;
    loadingMessagesRef.current = true;
    setLoadingMessages(true);
    try {
      const params = new URLSearchParams({ limit: String(MESSAGE_PAGE * 2), view: 'skeleton' });
      params.set('around', around);
      const res = await fetch(`/chat/messages/${encodeURIComponent(sessionId)}?${params}`);
      if (!res.ok) throw new Error(await res.text());
      const page: MessagePage = await res.json();
      if (req !== messageRequestRef.current || activeSessionIdRef.current !== sessionId) return;
      updateSessionMessageCount(sessionId, page.total);
      updateSessionBoundaryTimes(sessionId, page);
      const chunk = normalizeChatHistoryChunk<ChatMessage>(page.data || [], (raw) => normalizeMessage(raw, activeSession?.source));
      messagesRef.current = chunk;
      pendingJumpMessageIdRef.current = messageId;
      setMessages(chunk);
      hasOlderRef.current = Boolean(page.has_older);
      hasNewerRef.current = Boolean(page.has_newer);
      setHasOlder(Boolean(page.has_older));
      setHasNewer(Boolean(page.has_newer));
    } catch (err) { setStatus(tf('status.messagesUnavailable', errorMessage(err))); }
    finally {
      if (req === messageRequestRef.current) {
        loadingMessagesRef.current = false;
        setLoadingMessages(false);
      }
    }
  }, [activeSession?.source, updateSessionBoundaryTimes, updateSessionMessageCount]);

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
    } catch (err) { setWorkspaceEntries([]); setStatus(tf('workspace.unavailable', errorMessage(err))); }
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
    } catch (err) { setStatus(tf('workspace.folderUnavailable', errorMessage(err))); }
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
    if (!res.ok) { setStatus(tf('status.skillFileUnavailable', await res.text())); return; }
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
    } catch (err) { setStatus(tf('status.skillUnavailable', errorMessage(err))); }
  }, [loadSkillFiles, openSkillFile, writeHashRoute]);
  const loadSkills = useCallback(async () => {
    try {
      const res = await fetch('/skills/list', { cache: 'no-store' });
      if (!res.ok) throw new Error(await res.text());
      const body = await res.json();
      const list: Skill[] = body.data || body.skills || [];
      setSkillList(list);
    } catch (err) { setStatus(tf('status.skillsUnavailable', errorMessage(err))); }
  }, []);
  const toggleSkillEnabled = useCallback(async (skill: Skill, enabled: boolean) => {
    const res = await fetch(`/skills/toggle/${encodeURIComponent(skill.name)}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ enabled }) });
    if (!res.ok) { setStatus(tf('status.skillToggleFailed', await res.text())); return; }
    setSkillList((old) => old.map((item) => item.name === skill.name ? { ...item, enabled } : item));
    setStatus(tf(enabled ? 'status.skillEnabled' : 'status.skillDisabled', skill.name));
  }, []);
  const openSkillMenu = (skill: Skill, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.min(event.clientX, window.innerWidth - 190);
    const y = Math.min(event.clientY, window.innerHeight - 112);
    setSkillMenu({ skill, x: Math.max(8, x), y: Math.max(8, y) });
  };
  const deleteSkill = async (skill: Skill) => {
    setSkillMenu(null);
    if (!await requestConfirm(t('skills.deleteTitle'), tf('skills.deleteConfirm', skill.name), true)) return;
    const res = await fetch(`/skills/${encodeURIComponent(skill.name)}`, { method: 'DELETE' });
    if (!res.ok) { setStatus(tf('skills.deleteFailed', await res.text())); return; }
    setSkillList((old) => old.filter((item) => item.name !== skill.name));
    if (selectedSkillName === skill.name) {
      clearSelectedSkill();
      writeHashRoute({ mode: 'skills' });
    }
    await loadSkills();
    setStatus(t('skills.deleted'));
    showToast(t('skills.deleted'));
  };
  const skillEntryParentPath = (path: string) => path.split('/').filter(Boolean).slice(0, -1).join('/');
  const openSkillFileMenu = (skill: Skill, entry: WorkspaceEntry, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.min(event.clientX, window.innerWidth - 190);
    const y = Math.min(event.clientY, window.innerHeight - 160);
    setSkillFileMenu({ skill, entry, x: Math.max(8, x), y: Math.max(8, y) });
  };
  const renameSkillFileEntry = async (skill: Skill, entry: WorkspaceEntry) => {
    setSkillFileMenu(null);
    const nextName = await requestPrompt(t('skills.renameFileTitle'), t('skills.renameFileMessage'), entry.name);
    if (nextName === null) return;
    const res = await fetch(`/skills/item?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(entry.path)}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: nextName }) });
    if (!res.ok) { setStatus(tf('skills.renameFileFailed', await res.text())); return; }
    setSkillPreview((old) => old.path === entry.path || old.path.startsWith(`${entry.path}/`) ? { path: '', content: '', kind: 'none' } : old);
    await loadSkillFiles(skill, skillEntryParentPath(entry.path));
    setStatus(t('skills.renamedFile'));
    showToast(t('skills.renamedFile'));
  };
  const deleteSkillFileEntry = async (skill: Skill, entry: WorkspaceEntry) => {
    setSkillFileMenu(null);
    if (!await requestConfirm(t('skills.deleteFileTitle'), tf('skills.deleteFileConfirm', entry.kind, entry.name), true)) return;
    const res = await fetch(`/skills/item?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(entry.path)}`, { method: 'DELETE' });
    if (!res.ok) { setStatus(tf('skills.deleteFileFailed', await res.text())); return; }
    setSkillPreview((old) => old.path === entry.path || old.path.startsWith(`${entry.path}/`) ? { path: '', content: '', kind: 'none' } : old);
    await loadSkillFiles(skill, skillEntryParentPath(entry.path));
    setStatus(t('skills.deletedFile'));
    showToast(t('skills.deletedFile'));
  };
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
    } catch (err) { setStatus(tf('status.skillFolderUnavailable', errorMessage(err))); }
  }, [expandedSkillPaths, loadSkillFiles, selectedSkill]);

  const resetCronForm = useCallback(() => { setCronName(''); setCronSchedule('0 9 * * *'); setCronPrompt(''); setCronScript(''); setCronDeliver(''); setCronOutput(null); setCronEditingId(''); }, []);
  const loadCronJobs = useCallback(async () => {
    try {
      const res = await fetch(apiJoin(apiBase, '/api/jobs?include_disabled=true'), { headers: headers(false) });
      if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
      const body = await res.json();
      const nextJobs = body.data || body.jobs || [];
      setCronJobs(nextJobs);
      if (cronEditingId && !nextJobs.some((job: Job) => jobId(job) === cronEditingId)) resetCronForm();
    } catch (err) { setStatus(tf('cron.jobsUnavailable', errorMessage(err))); }
  }, [apiBase, cronEditingId, headers, resetCronForm]);
  const fetchCronOutput = useCallback(async (id: string) => {
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(id)}/output/latest`), { headers: headers(false) });
    if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
    const body = await res.json();
    return (body.output || body.data || null) as CronOutput | null;
  }, [apiBase, headers]);
  const loadCronOutput = useCallback(async (id = cronEditingId) => {
    if (!id) { setCronOutput(null); return; }
    setCronOutputLoading(true);
    try {
      setCronOutput(await fetchCronOutput(id));
    } catch (err) {
      setCronOutput({ content: tf('cron.outputUnavailable', errorMessage(err)) });
    } finally {
      setCronOutputLoading(false);
    }
  }, [cronEditingId, fetchCronOutput]);
  const beginCronEdit = useCallback((job: Job) => {
    const values = cronEditableValues(job);
    setCronEditingId(jobId(job));
    setCronName(values.name);
    setCronSchedule(values.schedule || '0 9 * * *');
    setCronPrompt(values.prompt);
    setCronScript(values.script);
    setCronDeliver(values.deliver);
    writeHashRoute({ mode: 'cron', jobId: jobId(job) });
  }, [writeHashRoute]);
  const saveCronJob = useCallback(async () => {
    if (!cronEditingId) {
      const body: { name: string; schedule: string; prompt: string; script?: string; deliver?: string } = { name: cronName, schedule: cronSchedule, prompt: cronPrompt };
      if (cronScript) body.script = cronScript;
      if (cronDeliver.trim()) body.deliver = cronDeliver;
      const res = await fetch(apiJoin(apiBase, '/api/jobs'), { method: 'POST', headers: headers(), body: JSON.stringify(body) });
      if (!res.ok) { setStatus(await res.text()); return; }
      const bodyJson = await res.json().catch(() => ({}));
      const created = (bodyJson.data || bodyJson.job || bodyJson) as Job;
      const id = jobId(created);
      if (id) setCronEditingId(id);
      setStatus(t('cron.saved'));
      showToast(t('cron.saved'));
      await loadCronJobs();
      return;
    }
    const patchBody = buildCronPatch({ name: cronName, schedule: cronSchedule, prompt: cronPrompt, script: cronScript, deliver: cronDeliver });
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}`), { method: 'PATCH', headers: headers(), body: JSON.stringify(patchBody) });
    if (!res.ok) { setStatus(await res.text()); return; }
    await loadCronJobs();
    setStatus(t('cron.saved'));
    showToast(t('cron.saved'));
  }, [apiBase, cronDeliver, cronEditingId, cronName, cronPrompt, cronSchedule, cronScript, headers, loadCronJobs, showToast]);
  const runCronJob = useCallback(async () => {
    if (!cronEditingId) return;
    const id = cronEditingId;
    const previousOutputTimestamp = cronOutput?.timestamp || '';
    setCronOutputLoading(true);
    try {
      const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(id)}/run`), { method: 'POST', headers: headers(false) });
      if (!res.ok) { setStatus(await res.text()); return; }
      setStatus(t('cron.ran'));
      showToast(t('cron.ran'));
      await loadCronJobs();
      const output = await waitForCronRunOutput(() => fetchCronOutput(id), previousOutputTimestamp);
      setCronOutput(output);
    } catch (err) {
      setCronOutput({ content: tf('cron.outputUnavailable', errorMessage(err)) });
    } finally {
      setCronOutputLoading(false);
    }
  }, [apiBase, cronEditingId, cronOutput?.timestamp, fetchCronOutput, headers, loadCronJobs, showToast]);
  const toggleCronPaused = useCallback(async () => {
    if (!cronEditingId) return;
    const currentJob = cronJobs.find((job) => jobId(job) === cronEditingId);
    if (!currentJob) return;
    const paused = jobState(currentJob) === 'paused';
    const action = paused ? 'resume' : 'pause';
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}/${action}`), { method: 'POST', headers: headers(false) });
    if (!res.ok) { setStatus(await res.text()); return; }
    await loadCronJobs();
    const message = t(paused ? 'cron.resumedDone' : 'cron.pausedDone');
    setStatus(message);
    showToast(message);
  }, [apiBase, cronEditingId, cronJobs, headers, loadCronJobs, showToast]);
  const deleteCronJob = useCallback(async () => {
    if (!cronEditingId) return;
    if (!await requestConfirm(t('cron.deleteTitle'), tf('cron.deleteConfirm', cronName || cronEditingId), true)) return;
    const res = await fetch(apiJoin(apiBase, `/api/jobs/${encodeURIComponent(cronEditingId)}`), { method: 'DELETE', headers: headers(false) });
    if (!res.ok) { setStatus(await res.text()); return; }
    resetCronForm();
    await loadCronJobs();
    setStatus(t('cron.deleted'));
    showToast(t('cron.deleted'));
  }, [apiBase, cronEditingId, cronName, headers, loadCronJobs, requestConfirm, resetCronForm, showToast]);

  useEffect(() => { loadModels(); loadWorkspace(''); }, []);
  useEffect(() => { if (mode === 'insights') loadUsageInsights(usagePeriod); }, [mode, usagePeriod, loadUsageInsights]);
  useEffect(() => { const t = window.setTimeout(() => loadSessions(filter), 180); return () => window.clearTimeout(t); }, [filter, loadSessions]);
  useEffect(() => { if (mode === 'cron') loadCronJobs(); }, [mode, loadCronJobs]);
  useEffect(() => { if (mode === 'cron' && cronEditingId) loadCronOutput(cronEditingId); else setCronOutput(null); }, [mode, cronEditingId, loadCronOutput]);
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
  useEffect(() => {
    if (!activeSessionId) return;
    loadSessionDetail(activeSessionId);
    if (skipNextHistoryLoadRef.current === activeSessionId) {
      skipNextHistoryLoadRef.current = '';
      return;
    }
    messageRequestRef.current += 1;
    userNavRequestRef.current += 1;
    contextWindowRequestRef.current += 1;
    messagesRef.current = [];
    hasOlderRef.current = false;
    hasNewerRef.current = false;
    pendingHistoryScrollAnchorRef.current = null;
    setMessages([]);
    setUserMessageNav([]);
    setContextWindowSnapshot(null);
    setHistoryTotal(null);
    setLatestReadySessionId('');
    setHasOlder(false);
    setHasNewer(false);
    const isDraft = activeSessionId === DRAFT_SESSION_ID;
    setUserNavLoading(!isDraft);
    if (isDraft) return;
    loadMessageWindow(activeSessionId, 'latest');
  }, [activeSessionId]);
  useEffect(() => {
    if (!latestReadySessionId || latestReadySessionId !== activeSessionId) return;
    void Promise.allSettled([
      loadUserMessageNav(latestReadySessionId),
      loadContextWindowSnapshot(latestReadySessionId),
    ]);
  }, [activeSessionId, latestReadySessionId, loadContextWindowSnapshot, loadUserMessageNav]);
  useEffect(() => {
    if (watchSourceRef.current) { watchSourceRef.current.close(); watchSourceRef.current = null; }
    clearNewMessages();
    if (!activeSessionId || activeSessionId === DRAFT_SESSION_ID) return;
    const watchedSessionId = activeSessionId;
    const es = new EventSource(`/chat/watch/${encodeURIComponent(watchedSessionId)}`);
    watchSourceRef.current = es;
    es.onmessage = (ev) => {
      try {
        if (activeSessionIdRef.current !== watchedSessionId) return;
        const raw = JSON.parse(ev.data);
        const msg = normalizeMessage(raw, activeSession?.source);
        const wasNearBottom = !!chatScrollRef.current && isNearBottom(chatScrollRef.current);
        const prev = messagesRef.current;
        const next = mergeWatchedMessage(prev, msg);
        messagesRef.current = next;
        if (wasNearBottom) {
          scrollLatestAfterRenderRef.current = true;
          clearNewMessages();
        } else {
          const previousVisible = visibleChatMessages(prev, showReasoningRef.current, showToolCallsRef.current);
          const nextVisible = visibleChatMessages(next, showReasoningRef.current, showToolCallsRef.current);
          const marker = computeNewMessageMarker(previousVisible, nextVisible, newMessageBoundaryIdRef.current);
          newMessageBoundaryIdRef.current = marker.firstId;
          setNewMessageBoundaryId(marker.firstId);
          setNewMessageCount(marker.count);
        }
        setMessages(next);
        loadContextWindowSnapshot(watchedSessionId);
        setStatus(t('chat.streamingOther'));
      } catch { /* ignore */ }
    };
    es.onerror = () => { watchSourceRef.current = null; };
    return () => { es.close(); watchSourceRef.current = null; };
  }, [activeSession?.source, activeSessionId, clearNewMessages, loadContextWindowSnapshot]);
  useEffect(() => { streamingSessionIdRef.current = streamingSessionId; }, [streamingSessionId]);
  useEffect(() => {
    if (!activeSessionId || activeSessionId === DRAFT_SESSION_ID) return;
    const targetSessionId = activeSessionId;
    let cancelled = false;
    const probe = async () => {
      const promise = (async () => {
        try {
          const res = await fetch(`/chat/stream/${encodeURIComponent(targetSessionId)}/status`, { headers: headers() });
          if (!res.ok) return null;
          const body = await res.json();
          return !!body?.running;
        } catch { return null; }
      })();
      streamStatusProbeRef.current = { sessionId: targetSessionId, promise };
      const running = await promise;
      if (streamStatusProbeRef.current?.promise === promise) streamStatusProbeRef.current = null;
      if (cancelled || running === null) return;
      if (running) {
        setStreamingSessionId(targetSessionId);
        setStatus(t('chat.streamingOther'));
      } else if (streamingSessionIdRef.current === targetSessionId) {
        setStreamingSessionId('');
      }
    };
    probe();
    const timer = window.setInterval(probe, 1500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      if (streamStatusProbeRef.current?.sessionId === targetSessionId) streamStatusProbeRef.current = null;
    };
  }, [activeSessionId, headers, t]);
  useEffect(() => {
    if (!sessionMenu && !skillMenu && !skillFileMenu && !workspaceMenu) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === 'Escape') { setSessionMenu(null); setSkillMenu(null); setSkillFileMenu(null); setWorkspaceMenu(null); } };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Element | null;
      if (target?.closest('.session-context-menu,.skill-context-menu,.skill-file-context-menu,.workspace-context-menu')) return;
      setSessionMenu(null);
      setSkillMenu(null);
      setSkillFileMenu(null);
      setWorkspaceMenu(null);
    };
    window.addEventListener('keydown', onKey);
    window.addEventListener('pointerdown', onPointerDown, true);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pointerdown', onPointerDown, true);
    };
  }, [sessionMenu, skillMenu, skillFileMenu, workspaceMenu]);
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
  useLayoutEffect(() => {
    const targetId = pendingJumpMessageIdRef.current;
    if (!targetId) return;
    pendingJumpMessageIdRef.current = '';
    const target = document.querySelector(`[data-message-id="${CSS.escape(targetId)}"]`);
    const scrollTarget = () => target?.scrollIntoView({ block: 'center' });
    scrollTarget();
    requestAnimationFrame(scrollTarget);
    window.setTimeout(scrollTarget, 60);
  }, [messages, activeSessionId]);
  useLayoutEffect(() => {
    const anchor = pendingHistoryScrollAnchorRef.current;
    if (!anchor) return;
    pendingHistoryScrollAnchorRef.current = null;
    const scroller = chatScrollRef.current;
    if (!scroller) return;
    const restore = () => restoreMessageScrollAnchor(scroller, anchor);
    restoreMessageScrollAnchor(scroller, anchor);
    requestAnimationFrame(restore);
    window.setTimeout(restore, 60);
  }, [messages]);

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
  const currentSessionStreaming = !!activeSessionId && streamingSessionId === activeSessionId;
  const stopStreaming = async () => {
    if (!activeSessionId) {
      chatAbortRef.current?.abort();
      return;
    }
    const requestStop = async () => {
      const res = await fetch(`/chat/stream/${encodeURIComponent(activeSessionId)}/stop`, { method: 'POST', headers: headers() });
      if (!res.ok) throw new Error(await res.text());
      return res.json() as Promise<{ status?: string }>;
    };
    try {
      let result = await requestStop();
      // A click can arrive while the stream request is still being admitted.
      // Retry once after the request has reached the backend; the backend then
      // records the pending stop and forwards it as soon as a run id exists.
      if (result.status === 'not_running' && chatAbortRef.current) {
        await new Promise((resolve) => window.setTimeout(resolve, 120));
        result = await requestStop();
      }
      if (result.status === 'stopping' || result.status === 'stop_pending') {
        chatAbortRef.current?.abort();
        setStatus(t('status.stopped'));
      }
    } catch (err) {
      setStatus(tf('status.error', errorMessage(err)));
    }
  };
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
    setStatus(tf('chat.queuedFollowUpStatus', `${trimmed.slice(0, 80)}${trimmed.length > 80 ? '…' : ''}`));
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
      const res = await fetch(apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(sessionId)}/chat`), { method: 'POST', headers: headers(), body: JSON.stringify(buildChatRequestBody(`/steer ${text}`, sessionModel, effort, sessionProvider)) });
      if (!res.ok) throw new Error(await res.text());
      setStatus(tf('chat.steeredStatus', `${trimmed.slice(0, 80)}${trimmed.length > 80 ? '…' : ''}`));
    } catch (err) {
      setStatus(tf('chat.steerFailed', errorMessage(err)));
      enqueueFollowUp(trimmed, sessionId);
    }
  };

  const runChatTurn = async (turnText: string, turnAttachments: Attachment[], initialSessionId = activeSessionId, clearComposer = true) => {
    const text = turnText.trim();
    if (!text && turnAttachments.length === 0) return;
    messageRequestRef.current += 1;
    setBusy(true); setStatus(t('status.running'));
    let sessionId = initialSessionId;
    let effectiveSessionId = sessionId;
    let createdSession: Session | null = null;
    try {
      if (!sessionId || sessionId === DRAFT_SESSION_ID) {
        createdSession = await createSession();
        sessionId = createdSession.id;
        effectiveSessionId = sessionId;
        skipNextHistoryLoadRef.current = sessionId;
        activeSessionIdRef.current = sessionId;
        setActiveSessionId(sessionId);
        setActiveSessionDetail(createdSession);
        setSessions((old) => old.some((s) => s.id === sessionId) ? old.map((s) => s.id === sessionId ? { ...s, ...createdSession } : s) : [createdSession!, ...old]);
        writeHashRoute({ mode: 'chat', sessionId });
        setHasOlder(false);
        setHasNewer(false);
      }
    } catch (err) { setStatus(tf('status.cannotCreateSession', errorMessage(err))); setBusy(false); return; }
    const stick = isNearBottom(chatScrollRef.current, 180);
    let payloadAttachments: Attachment[] = turnAttachments;
    try {
      payloadAttachments = await uploadAttachments(turnAttachments);
    } catch (err) {
      setStatus(tf('status.cannotUploadAttachments', errorMessage(err)));
      setBusy(false);
      return;
    }
    const userText = text || payloadAttachments.map((a) => a.name).join(', ');
    const sessionOverride = sessionModelOverridesRef.current[sessionId];
    const sessionModel = sessionOverride?.model || realModelOrEmpty(modelRef.current) || createdSession?.model || activeSession?.model || activeSessionDetail?.model || '';
    const sessionProvider = resolveModelProvider(modelsRef.current, sessionModel, sessionOverride?.provider ?? (providerRef.current || createdSession?.provider || activeSession?.provider || activeSessionDetail?.provider || ''));
    const userMsg: ChatMessage = { id: uid('user'), role: 'user', content: userText, timestamp: Date.now() / 1000 };
    const assistantId = uid('assistant');
    const assistantMsg: ChatMessage = { id: assistantId, role: 'assistant', content: '', pending: true, timestamp: Date.now() / 1000, model: sessionModel, provider: sessionProvider };
    const payloadInput = buildPayload(text, payloadAttachments);
    if (createdSession) setMessages(() => [userMsg, assistantMsg]);
    else setMessages((old) => [...old, userMsg, assistantMsg].slice(-MESSAGE_WINDOW));
    setHasNewer(false);
    if (clearComposer) { setInput(''); setAttachments([]); }
    setStatus(t('status.running'));
    if (stick) requestAnimationFrame(() => { if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; });
    let controller: AbortController | null = null;
    try {
      controller = new AbortController();
      chatAbortRef.current = controller;
      setStreamingSessionId(sessionId);
      const res = await fetch(`/chat/stream/${encodeURIComponent(sessionId)}`, { method: 'POST', headers: headers(), body: JSON.stringify(buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)), signal: controller.signal });
      if (!res.ok || !res.body) throw new Error(await res.text());
      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';
      let finalText = '';
      let reasoningText = '';
      let turnMetrics: ChatTurnMetrics | undefined;
      const scrollWithStream = () => {
        if (isNearBottom(chatScrollRef.current, 220)) requestAnimationFrame(() => { if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; });
      };
      const animator = createStreamAnimator({
        onUpdate: (text) => {
          setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, content: text, pending: true, timestamp: Date.now() / 1000 } : m));
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
            const payloadSessionId = typeof payload?.session_id === 'string' ? payload.session_id.trim() : '';
            if (payloadSessionId && payloadSessionId !== effectiveSessionId) effectiveSessionId = payloadSessionId;
            turnMetrics = mergeTurnMetrics(turnMetrics, readTurnMetrics(payload));
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
              turnMetrics = mergeTurnMetrics(turnMetrics, readTurnMetrics(payload.messages?.[0]));
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
      sessionId = await reconcileEffectiveSession(sessionId, effectiveSessionId, createdSession);
      setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, pending: false, content: finalText || m.content, reasoning: reasoningText || m.reasoning, timestamp: Date.now() / 1000, turnMetrics: turnMetrics } : m));
      setStatus(t('chat.connected'));
      await refreshSessionTitleOnce(sessionId);
      await loadWorkspace(workspacePath);
    } catch (err) {
      if (isAbortError(err)) {
        setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, pending: false, content: m.content } : m));
        setStatus(t('status.stopped'));
      } else {
        setMessages((old) => old.map((m) => m.id === assistantId ? { ...m, pending: false, content: `Request failed: ${errorMessage(err)}` } : m));
        setStatus(tf('status.error', errorMessage(err)));
      }
    } finally {
      setBusy(false);
      if (chatAbortRef.current === controller) chatAbortRef.current = null;
      setStreamingSessionId((current) => current === sessionId ? '' : current);
      const nextQueued = shiftNextFollowUp(sessionId);
      if (nextQueued) window.setTimeout(() => runChatTurn(nextQueued.text, [], sessionId, false), 0);
    }
  };

  const sendMessage = async () => {
    const text = input.trim();
    if (!text && attachments.length === 0) return;
    clearNewMessages();
    const sessionId = activeSessionId;
    const pendingStreamProbe = streamStatusProbeRef.current;
    const remoteSessionStreaming = pendingStreamProbe?.sessionId === sessionId && await pendingStreamProbe.promise === true;
    if (currentSessionStreaming || remoteSessionStreaming) {
      if (!text) return;
      if (followUpBehaviour === 'steer') await steerFollowUp(text);
      else enqueueFollowUp(text, sessionId);
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
    const res = await fetch(`/workspace/file?path=${encodeURIComponent(entry.path)}&preview=1`);
    if (!res.ok) { setStatus(tf('workspace.previewFailed', res.status)); return; }
    const blob = await res.blob();
    if (blob.type.startsWith('image/')) {
      setPreview({ path: entry.path, content: '', kind: 'image', url: URL.createObjectURL(blob) });
      if (options?.edit) setStatus(t('workspace.itemNotEditable'));
    } else if (blob.type.startsWith('text/') || isWorkspaceTextFile(entry.name)) {
      setPreview({ path: entry.path, content: await blob.text(), kind: 'text', editRequest: options?.edit ? Date.now() : undefined });
    } else {
      const totalSize = Number(res.headers.get('x-yahu-file-size')) || entry.size || blob.size;
      setPreview({
        path: entry.path,
        content: formatHexDump(new Uint8Array(await blob.arrayBuffer())),
        kind: 'hex',
        totalSize,
        truncated: res.headers.get('x-yahu-preview-truncated') === '1',
      });
      if (options?.edit) setStatus(t('workspace.itemNotEditable'));
    }
    if (options?.route !== false) writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: entry.path });
  }, [toggleWorkspaceFolder, writeHashRoute]);
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
    openWorkspaceRouteTarget(workspaceRouteTarget.workspacePath, workspaceRouteTarget.workspaceKind, { edit: workspaceRouteTarget.workspaceEdit }).catch((err) => setStatus(tf('workspace.routeUnavailable', errorMessage(err))));
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
    const res = await fetch(`/sessions/${encodeURIComponent(session.id)}/title`, { method: 'PATCH', headers: headers(), body: JSON.stringify({ title: nextTitle }) });
    if (!res.ok) { setStatus(`Rename failed: ${await res.text()}`); return; }
    const body = await res.json();
    const titles = body.titles && typeof body.titles === 'object' ? body.titles as Record<string, string> : {};
    renamedSessionTitlesRef.current = { ...renamedSessionTitlesRef.current, ...titles };
    const updatedIds = new Set<string>(Array.isArray(body.updated_ids) ? body.updated_ids : [session.id]);
    setSessions((old) => old.map((item) => updatedIds.has(item.id) ? { ...item, title: titles[item.id] || body.title || nextTitle } : item));
    setActiveSessionDetail((old) => old && updatedIds.has(old.id) ? { ...old, title: titles[old.id] || body.title || nextTitle } : old);
    await loadSessions(filter);
    setStatus(t('status.renamedSession'));
  };
  const deleteSession = async (session: Session) => {
    setSessionMenu(null);
    if (!await requestConfirm(t('chat.deleteTitle'), t('chat.deleteConfirm'), true)) return;
    const res = await fetch(apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(session.id)}`), { method: 'DELETE', headers: headers(false) });
    if (!res.ok) { setStatus(`Delete failed: ${await res.text()}`); return; }
    setPinnedIds((old) => { const next = new Set(old); next.delete(session.id); return next; });
    setSessions((old) => {
      const next = old.filter((item) => item.id !== session.id);
      if (activeSessionId === session.id) switchActiveSession(next[0]?.id || '');
      return next;
    });
    if (activeSessionId === session.id) { setMessages([]); setActiveSessionDetail(null); }
    await loadSessions(filter);
    setStatus(t('status.deletedSession'));
  };
  const openWorkspaceMenu = (entry: WorkspaceEntry, event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.min(event.clientX, window.innerWidth - 190);
    const y = Math.min(event.clientY, window.innerHeight - (entry.kind === 'file' ? 220 : 156));
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
  const openWorkspaceTerminal = (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    setTerminalMounted(true);
    setTerminalCwd(entry.path);
    setMode('terminal');
    setSidebarCollapsed(true);
    setMobileSidebarOpen(false);
    writeHashRoute({ mode: 'terminal', cwd: entry.path });
  };
  const renameWorkspaceEntry = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    const nextName = await requestPrompt(t('workspace.renameTitle'), t('workspace.renameMessage'), entry.name);
    if (nextName === null) return;
    const res = await fetch(`/workspace/item?path=${encodeURIComponent(entry.path)}`, { method: 'PATCH', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: nextName }) });
    if (!res.ok) { setStatus(tf('workspace.renameFailed', await res.text())); return; }
    setPreview((old) => old.path === entry.path ? { path: '', content: '', kind: 'none' } : old);
    await loadWorkspace(workspacePath);
    setStatus(t('workspace.renamed'));
  };
  const deleteWorkspaceEntry = async (entry: WorkspaceEntry) => {
    setWorkspaceMenu(null);
    if (!await requestConfirm(t('workspace.deleteTitle'), tf('workspace.deleteConfirm', entry.kind, entry.name), true)) return;
    const res = await fetch(`/workspace/item?path=${encodeURIComponent(entry.path)}`, { method: 'DELETE' });
    if (!res.ok) { setStatus(tf('workspace.deleteFailed', await res.text())); return; }
    setPreview((old) => old.path === entry.path || old.path.startsWith(`${entry.path}/`) ? { path: '', content: '', kind: 'none' } : old);
    await loadWorkspace(workspacePath);
    setStatus(t('workspace.deleted'));
  };
  const applySidebarWidth = useCallback((nextWidth: number) => {
    sidebarWidthRef.current = nextWidth;
    setSidebarWidth(nextWidth);
  }, []);
  const beginSidebarResize = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    if (sidebarCollapsed || window.matchMedia('(max-width: 900px)').matches) return;
    const shell = event.currentTarget.closest('.app-shell');
    if (!(shell instanceof HTMLElement)) return;
    event.preventDefault();
    sidebarResizeRef.current = { pointerId: event.pointerId, left: shell.getBoundingClientRect().left };
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* continue without pointer capture */ }
    setSidebarResizing(true);
  }, [sidebarCollapsed]);
  const resizeSidebar = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const active = sidebarResizeRef.current;
    if (!active || active.pointerId !== event.pointerId) return;
    applySidebarWidth(sidebarWidthFromPointer(event.clientX, active.left));
  }, [applySidebarWidth]);
  const endSidebarResize = useCallback((event: React.PointerEvent<HTMLDivElement>) => {
    const active = sidebarResizeRef.current;
    if (!active || active.pointerId !== event.pointerId) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
    sidebarResizeRef.current = null;
    setSidebarResizing(false);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidthRef.current));
  }, []);
  const resizeSidebarWithKeyboard = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    const nextWidth = sidebarWidthFromKey(sidebarWidthRef.current, event.key);
    if (nextWidth === sidebarWidthRef.current) return;
    event.preventDefault();
    applySidebarWidth(nextWidth);
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(nextWidth));
  }, [applySidebarWidth]);
  const closeMobileSidebar = () => setMobileSidebarOpen(false);
  const toggleMobileSidebar = () => {
    if (!hasMobileDrawer(mode)) return;
    setSidebarCollapsed(false);
    setMobileSidebarOpen((value) => !value);
  };
  const setNavMode = (next: Mode, collapse = false) => {
    if (next === 'terminal') setTerminalMounted(true);
    setMode(next);
    setSidebarCollapsed(collapse || next === 'memory' || next === 'insights' || next === 'terminal' || next === 'settings');
    setMobileSidebarOpen(false);
    const route: HashRoute = next === 'terminal' ? { mode: 'terminal', cwd: terminalCwd || undefined } : { mode: next } as HashRoute;
    writeHashRoute(route);
  };
  const wideMode = mode !== 'chat';
  const activeCronJob = cronJobs.find((job) => jobId(job) === cronEditingId) || null;

  const activeSessionModelOverride = activeSessionId ? sessionModelOverrides[activeSessionId] : undefined;
  return (
    <div className={`app-shell ${codeWrap ? 'code-wrap' : 'code-nowrap'} ${wideMode ? 'wide-mode' : ''} ${mode === 'images' ? 'image-mode' : ''} ${mode === 'skills' ? 'skills-mode' : ''} ${mode === 'memory' ? 'memory-mode' : ''} ${sidebarCollapsed ? 'nav-collapsed' : ''} ${mode === 'chat' && workspaceCollapsed ? 'workspace-collapsed' : ''} ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''} ${sidebarResizing ? 'sidebar-resizing' : ''}`} style={{ '--sidebar-width': `${sidebarWidth}px` } as React.CSSProperties}>
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
          <button className={`rail-btn nav-terminal ${mode === 'terminal' ? 'active' : ''}`} onClick={() => setNavMode('terminal', true)} title={t('nav.terminal')}><Terminal /></button>
          <button className={`rail-btn nav-settings ${mode === 'settings' ? 'active' : ''}`} onClick={() => setNavMode('settings')} title={t('nav.settings')}><Settings /></button>
        </div>
        {!sidebarCollapsed && <div className="left-body">
          {mode === 'chat' ? <ChatSidebar filter={filter} setFilter={setFilter} hideCronSessions={hideCronSessions} setHideCronSessions={(value: boolean) => setHideCronSessions(value)} startDraftSession={startDraftSession} pinnedSessions={filteredSessions.pinned} normalSessions={filteredSessions.normal} activeSessionId={activeSessionId} setActiveSessionId={switchActiveSession} writeHashRoute={writeHashRoute} closeMobileSidebar={closeMobileSidebar} pinnedIds={pinnedIds} togglePin={togglePin} openSessionMenu={openSessionMenu} openSessionMenuAt={openSessionMenuAt} /> : mode === 'cron' ? <CronSidebar jobs={cronJobs} editingId={cronEditingId} beginCronEdit={beginCronEdit} resetCronForm={resetCronForm} writeHashRoute={writeHashRoute} closeMobileSidebar={closeMobileSidebar} /> : mode === 'workspace' ? <WorkspaceSidebar rootEntries={workspaceTree[''] || workspaceEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openWorkspaceEntry={openWorkspaceEntry} downloadEntry={downloadEntry} openWorkspaceMenu={openWorkspaceMenu} /> : mode === 'skills' ? <SkillsSidebar skills={skillList} activeSkillName={selectedSkillName} selectSkill={selectSkill} toggleSkillEnabled={toggleSkillEnabled} openSkillMenu={openSkillMenu} filter={skillFilter} setFilter={setSkillFilter} expandedCats={expandedSkillCats} setExpandedCats={setExpandedSkillCats} closeMobileSidebar={closeMobileSidebar} /> : (mode === 'memory' || mode === 'settings') ? null : <ModeSidebar mode={mode} />}
        </div>}
        {!sidebarCollapsed && <ThemeCard theme={theme} setTheme={setTheme} />}
      </aside>
      <div className="sidebar-resize-handle" role="separator" aria-label={t('nav.resizeSidebar')} aria-orientation="vertical" aria-valuemin={MIN_SIDEBAR_WIDTH} aria-valuemax={MAX_SIDEBAR_WIDTH} aria-valuenow={sidebarWidth} tabIndex={sidebarCollapsed ? -1 : 0} onPointerDown={beginSidebarResize} onPointerMove={resizeSidebar} onPointerUp={endSidebarResize} onPointerCancel={endSidebarResize} onKeyDown={resizeSidebarWithKeyboard} />
      {mobileSidebarOpen && <button type="button" className="mobile-sidebar-backdrop" aria-label={t('nav.closeList')} onClick={closeMobileSidebar} />}
      {sessionMenu && <div className="session-context-menu" role="menu" style={{ left: sessionMenu.x, top: sessionMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        <button type="button" role="menuitem" onClick={() => renameSession(sessionMenu.session)}><Pencil /> {t('chat.rename')}</button>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteSession(sessionMenu.session)}><Trash2 /> {t('chat.delete')}</button>
      </div>}

      {skillMenu && <div className="skill-context-menu" role="menu" style={{ left: skillMenu.x, top: skillMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteSkill(skillMenu.skill)}><Trash2 /> {t('skills.delete')}</button>
      </div>}
      {skillFileMenu && <div className="skill-file-context-menu" role="menu" style={{ left: skillFileMenu.x, top: skillFileMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        <button type="button" role="menuitem" onClick={() => renameSkillFileEntry(skillFileMenu.skill, skillFileMenu.entry)}><Pencil /> {t('skills.renameFile')}</button>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteSkillFileEntry(skillFileMenu.skill, skillFileMenu.entry)}><Trash2 /> {t('skills.deleteFile')}</button>
      </div>}
      {workspaceMenu && <div className="workspace-context-menu" role="menu" style={{ left: workspaceMenu.x, top: workspaceMenu.y }} onContextMenu={(event) => event.preventDefault()}>
        {workspaceMenu.entry.kind === 'file' && <><button type="button" role="menuitem" onClick={() => viewWorkspaceEntry(workspaceMenu.entry)}><Eye /> {t('workspace.viewItem')}</button><button type="button" role="menuitem" onClick={() => editWorkspaceEntryPage(workspaceMenu.entry)}><Pencil /> {t('workspace.editItemPage')}</button></>}
        {workspaceMenu.entry.kind === 'dir' && <button type="button" role="menuitem" onClick={() => openWorkspaceTerminal(workspaceMenu.entry)}><Terminal /> {t('workspace.openInTerminal')}</button>}
        <button type="button" role="menuitem" onClick={() => renameWorkspaceEntry(workspaceMenu.entry)}><Pencil /> {t('workspace.renameItem')}</button>
        <button type="button" role="menuitem" className="danger" onClick={() => deleteWorkspaceEntry(workspaceMenu.entry)}><Trash2 /> {t('workspace.deleteItem')}</button>
      </div>}

      {mode === 'chat' && <>
        <ChatMain sessions={sessions} activeSessionDetail={activeSessionDetail} activeSessionModelOverride={activeSessionModelOverride} activeSessionId={activeSessionId} messages={messages} userMessageNav={userMessageNav} userNavLoading={userNavLoading} historyTotal={historyTotal} onJumpToMessage={jumpToMessage} contextWindowSnapshot={contextWindowSnapshot} showReasoning={showReasoning} setShowReasoning={setShowReasoning} desktopCompactMessages={desktopCompactMessages} setDesktopCompactMessages={setDesktopCompactMessages} showToolCalls={showToolCalls} setShowToolCalls={setShowToolCalls} hasOlder={hasOlder} hasNewer={hasNewer} loadingMessages={loadingMessages} loadMessageWindow={loadMessageWindow} attachments={attachments} setAttachments={setAttachments} input={input} setInput={setInput} onFiles={onFiles} fileInput={fileInput} sendMessage={sendMessage} stopStreaming={stopStreaming} composerEnterMode={composerEnterMode} model={model} selectedModelProvider={selectedModelProvider} setModel={changeSessionModel} models={models} effort={effort} setEffort={setEffort} busy={busy} streaming={currentSessionStreaming} followUpQueue={followUpQueue} onSteerQueuedItem={steerQueuedItem} onEditQueuedItem={editQueuedItem} onReorderQueuedItem={reorderQueuedItem} chatScrollRef={chatScrollRef} composerRef={composerRef} composerCompact={composerCompact} setComposerCompact={setComposerCompact} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} newMessageCount={newMessageCount} newMessageBoundaryId={newMessageBoundaryId} onClearNewMessages={() => { clearNewMessages(); if (chatScrollRef.current) chatScrollRef.current.scrollTop = chatScrollRef.current.scrollHeight; }} />
        <WorkspaceAside rootEntries={workspaceTree[''] || workspaceEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openWorkspaceEntry={openWorkspaceEntry} downloadEntry={downloadEntry} preview={preview} setPreview={setPreview} collapsed={workspaceCollapsed} setCollapsed={setWorkspaceCollapsed} openWorkspaceMenu={openWorkspaceMenu} openFullPreview={(path) => { setMode('workspace'); setSidebarCollapsed(false); writeHashRoute({ mode: 'workspace', workspaceKind: 'file', workspacePath: path }); setWorkspaceRouteTarget({ workspaceKind: 'file', workspacePath: path }); }} />
      </>}
      {mode === 'images' && <ImageBrowser theme={theme} setTheme={setTheme} requestConfirm={requestConfirm} initialImageFilename={initialImageFilename} writeHashRoute={writeHashRoute} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}

      {mode === 'workspace' && <WorkspaceMain preview={preview} setPreview={setPreview} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'skills' && <>
        <SkillMain skill={selectedSkill} preview={skillPreview} setPreview={setSkillPreview} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} showToast={showToast} />
        <SkillWorkspaceAside skill={selectedSkill} skillFileTree={skillFileTree} expandedSkillPaths={expandedSkillPaths} toggleSkillFolder={toggleSkillFolder} openSkillFile={openSkillFile} openSkillFileMenu={openSkillFileMenu} />
      </>}
      {mode === 'cron' && <CronMain name={cronName} setName={setCronName} schedule={cronSchedule} setSchedule={setCronSchedule} prompt={cronPrompt} setPrompt={setCronPrompt} script={cronScript} setScript={setCronScript} deliver={cronDeliver} setDeliver={setCronDeliver} editingId={cronEditingId} currentJob={activeCronJob} cronOutput={cronOutput} cronOutputLoading={cronOutputLoading} refreshCronOutput={() => loadCronOutput(cronEditingId)} saveCronJob={saveCronJob} runCronJob={runCronJob} toggleCronPaused={toggleCronPaused} deleteCronJob={deleteCronJob} theme={theme} setTheme={setTheme} mobileSidebarOpen={mobileSidebarOpen} toggleMobileSidebar={toggleMobileSidebar} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'memory' && <AdminMain mode={mode} apiBase={apiBase} headers={headers} setStatus={setStatus} showToast={showToast} theme={theme} setTheme={setTheme} onNavigateToSettings={() => setNavMode('settings')} />}
      {mode === 'insights' && <InsightsMain insights={usageInsights} loading={usageLoading} error={usageError} period={usagePeriod} setPeriod={setUsagePeriod} metric={usageMetric} setMetric={setUsageMetric} refresh={() => loadUsageInsights(usagePeriod, true)} theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={() => setNavMode('settings')} />}
      {terminalMounted && <Suspense fallback={mode === 'terminal' ? <main className="main-panel terminal-main"><div className="terminal-loading">{t('terminal.loading')}</div></main> : null}><WebTerminal active={mode === 'terminal'} cwd={terminalCwd} theme={theme} headerActions={<HeaderToolstrip theme={theme} setTheme={setTheme} mode={mode} onNavigateToTerminal={() => setNavMode('terminal', true)} onNavigateToSettings={() => setNavMode('settings')} />} /></Suspense>}
      {mode === 'settings' && <SettingsMain apiServerUrl={apiServerUrl} apiBase={apiBase} setApiBase={setApiBase} apiKey={apiKey} setApiKey={setApiKey} loadModels={loadModels} loadSessions={loadSessions} theme={theme} setTheme={setTheme} lang={lang} setLang={setLangState} followUpBehaviour={followUpBehaviour} setFollowUpBehaviour={setFollowUpBehaviour} composerEnterMode={composerEnterMode} setComposerEnterMode={setComposerEnterMode} codeWrap={codeWrap} setCodeWrap={setCodeWrap} showToast={showToast} />}
      <CustomDialog dialog={dialog} setDialog={setDialog} />
      <StatusToast message={toastMessage} />
      <nav className="mobile-bottom-nav" aria-label={t('nav.mobile')}>
        <button className={`rail-btn nav-chat ${mode === 'chat' ? 'active' : ''}`} onClick={() => setNavMode('chat')} aria-label={t('nav.chat')}><MessageSquare /></button>
        <button className={`rail-btn nav-cron ${mode === 'cron' ? 'active' : ''}`} onClick={() => setNavMode('cron')} aria-label={t('nav.cron')}><CalendarClock /></button>
        <button className={`rail-btn nav-skills ${mode === 'skills' ? 'active' : ''}`} onClick={() => setNavMode('skills')} aria-label={t('nav.skills')}><Puzzle /></button>
        <button className={`rail-btn nav-insights ${mode === 'insights' ? 'active' : ''}`} onClick={() => setNavMode('insights', true)} aria-label={t('nav.insights')}><LineChart /></button>
        <button className={`rail-btn nav-images ${mode === 'images' ? 'active' : ''}`} onClick={() => setNavMode('images', true)} aria-label={t('nav.images')}><ImageIcon /></button>
        <button className={`rail-btn nav-memory ${mode === 'memory' ? 'active' : ''}`} onClick={() => setNavMode('memory')} aria-label={t('nav.memory')}><Brain /></button>
      </nav>
    </div>
  );
}

function StatusToast({ message }: { message: string }) {
  if (!message) return null;
  return <div className="status-toast" role="status" aria-live="polite">{message}</div>;
}

function CustomDialog({ dialog, setDialog }: { dialog: DialogState; setDialog: (dialog: DialogState) => void }) {
  const [value, setValue] = useState('');
  const finish = useCallback((result: string | boolean | null) => { if (dialog) { dialog.resolve(result); setDialog(null); } }, [dialog, setDialog]);
  useEffect(() => { setValue(dialog?.value || ''); }, [dialog]);
  useEffect(() => {
    if (!dialog) return;
    const consumeDialogKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { consumeDialogKey(event); finish(dialog.variant === 'confirm' ? false : null); }
      if (event.key === 'Enter') { consumeDialogKey(event); finish(dialog.variant === 'prompt' ? (document.querySelector<HTMLInputElement>('.dialog-card input')?.value ?? '') : true); }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [dialog, finish]);
  if (!dialog) return null;
  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) finish(dialog.variant === 'confirm' ? false : null); }}>
    <form className="dialog-card" role="dialog" aria-modal="true" aria-label={dialog.title} onSubmit={(event) => { event.preventDefault(); }}>
      <h2>{dialog.title}</h2>
      <p>{dialog.message}</p>
      {dialog.variant === 'prompt' && <input autoFocus value={value} onChange={(event) => setValue(event.target.value)} />}
      <div className="dialog-actions">
        <button type="button" onClick={() => finish(dialog.variant === 'confirm' ? false : null)}>{t('dialog.cancel')}</button>
        <button type="button" className={dialog.danger ? 'danger' : ''} onClick={() => finish(dialog.variant === 'prompt' ? value : true)}>{dialog.variant === 'prompt' ? t('dialog.save') : t('dialog.confirm')}</button>
      </div>
    </form>
  </div>;
}
function ThemeCard({ theme, setTheme }: { theme: Theme; setTheme: (v: Theme) => void }) {
  return <div className="theme-card"><div className="theme-title"><span>{t('theme.appearance')}</span><span>{themeLabel(theme)}</span></div><label><span>{t('theme.theme')}</span><select value={theme} onChange={(e) => setTheme(e.target.value as Theme)}>{THEME_OPTIONS.map((s) => <option key={s.id} value={s.id}>{s.label}</option>)}</select></label></div>;
}
function MobileHeaderDrawerButton({ open, onClick }: { open: boolean; onClick: () => void }) {
  return <button type="button" className="mobile-header-drawer rail-btn" aria-label={t('nav.openList')} aria-expanded={open} onClick={onClick}><List /></button>;
}
function HeaderThemeControl({ theme, setTheme, mode, onNavigateToTerminal, onNavigateToSettings }: { theme: Theme; setTheme: (v: Theme) => void; mode?: Mode; onNavigateToTerminal?: () => void; onNavigateToSettings?: () => void }) {
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
  const navigateWithHistory = (route: HashRoute) => {
    if (pushHashRoute(window.history, window.location.hash, route)) window.dispatchEvent(new PopStateEvent('popstate'));
  };
  const navigateToTerminal = onNavigateToTerminal || (() => navigateWithHistory({ mode: 'terminal' }));
  const navigateToSettings = onNavigateToSettings || (() => navigateWithHistory({ mode: 'settings' }));
  const isTerminalActive = mode === 'terminal';
  const isSettingsActive = mode === 'settings';
  return <div className="header-theme-control" ref={rootRef}>
    <button type="button" className={`mobile-header-terminal-btn rail-btn nav-terminal ${isTerminalActive ? 'active' : ''}`} aria-label={t('nav.terminal')} onClick={navigateToTerminal}><Terminal /></button>
    <button type="button" className={`mobile-header-settings-btn rail-btn nav-settings ${isSettingsActive ? 'active' : ''}`} aria-label={t('settings.title')} onClick={navigateToSettings}><Settings /></button>
    <button type="button" className="mobile-icon-only palette-btn desktop-only-theme" aria-label={t('theme.theme')} aria-expanded={open} onClick={() => setOpen((value) => !value)}><Palette /></button>
    {open && <div className="theme-menu" role="menu">
      {THEME_OPTIONS.map((item) => <button key={item.id} type="button" role="menuitemradio" aria-checked={theme === item.id} className={theme === item.id ? 'active' : ''} onClick={() => { setTheme(item.id); setOpen(false); }}><span>{item.label}</span></button>)}
    </div>}
  </div>;
}
type HeaderToolstripProps = React.PropsWithChildren<{ className?: string; theme: Theme; setTheme: (v: Theme) => void; mode?: Mode; onNavigateToTerminal?: () => void; onNavigateToSettings?: () => void }>;
function HeaderToolstrip({ children, className = '', theme, setTheme, mode, onNavigateToTerminal, onNavigateToSettings }: HeaderToolstripProps) {
  const hasLeadingActions = React.Children.count(children) > 0;
  return <div className={`header-actions header-toolstrip${hasLeadingActions ? ' header-toolstrip-with-leading' : ''}${className ? ` ${className}` : ''}`}>{children}<HeaderThemeControl theme={theme} setTheme={setTheme} mode={mode} onNavigateToTerminal={onNavigateToTerminal} onNavigateToSettings={onNavigateToSettings} /></div>;
}
function ModeSidebar({ mode }: { mode: Mode }) {
  return <div className="admin-side"><h2>{navLabel(mode)}</h2><p>{modeSummary(mode)}</p></div>;
}

function InsightsMain(props: { insights: UsageInsights | null; loading: boolean; error: string; period: 1 | 7 | 30; setPeriod: (value: 1 | 7 | 30) => void; metric: UsageMetric; setMetric: (value: UsageMetric) => void; refresh: () => void; theme: Theme; setTheme: (value: Theme) => void; mode: Mode; onNavigateToSettings: () => void }) {
  const [chartStacked, setChartStacked] = useState(false);
  const [modelQuery, setModelQuery] = useState('');
  const periodData = props.insights?.periods?.find((item) => item.days === props.period);
  const totals = periodData?.totals || emptyTotals();
  const allModels = useMemo(() => (props.insights?.models || [])
    .map((model) => ({ ...model, periodTotals: finalizeTotals(modelPeriodTotals(model, props.period)) }))
    .filter((model) => model.periodTotals.total_tokens > 0)
    .sort((a, b) => b.periodTotals.total_tokens - a.periodTotals.total_tokens), [props.insights, props.period]);
  const models = useMemo(() => allModels.slice(0, 10), [allModels]);
  const filteredModels = useMemo(() => {
    const query = modelQuery.trim().toLocaleLowerCase();
    return allModels.filter((model) => {
      const label = usageModelLabel(model);
      return !query || label.toLocaleLowerCase().includes(query);
    });
  }, [allModels, modelQuery]);
  const topModel = models[0];
  const activeDays = periodSlice(props.insights?.daily || [], props.period);
  const activeHours = props.insights?.hourly || [];
  const activeSources = periodSources(props.insights?.periods || [], props.insights?.sources || [], props.period);
  const periodLabel = `${props.period}d`;
  const coverageLabel = props.insights?.coverage_started_at && !props.insights.coverage_complete
    ? tf('insights.trackingSince', formatInsightCoverageStart(props.insights.coverage_started_at))
    : '';
  const isSingleDay = props.period === 1;
  const showSkeleton = props.loading;
  const fmtCost = (value: number | undefined) => fmtMoney(value);
  const costMetricLabel = usageMetricLabel('cost_usd');
  return <main className={`main-panel insights-main ${showSkeleton ? 'insights-loading' : ''}`}>
    <header className="chat-header header-no-drawer insights-header">
      <div><h1>{t('insights.title')}</h1><span>{showSkeleton ? t('insights.loadingUsage') : props.error || tf('insights.lastTokens', periodLabel, fmtTokens(totals.total_tokens))}</span></div>
      <HeaderToolstrip theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings}><button className="icon-btn mobile-icon-only insights-refresh" onClick={props.refresh} disabled={props.loading} title={t('insights.refreshUsage')}><RefreshCw /></button></HeaderToolstrip>
    </header>
    <section className="insights-content">
      <div className="insights-toolbar" aria-label={t('insights.usageControls')}>
        <div className="segmented">{([30, 7, 1] as const).map((days) => <button key={days} className={props.period === days ? 'active' : ''} onClick={() => props.setPeriod(days)}>{days}d</button>)}</div>
        <select aria-label={t('insights.usageMetric')} value={props.metric} onChange={(event) => props.setMetric(event.target.value as UsageMetric)}>{(Object.keys(metricLabels) as UsageMetric[]).map((metric) => <option key={metric} value={metric}>{usageMetricLabel(metric)}</option>)}</select>
      </div>
      <div className="insights-cards">
        {showSkeleton ? <>
          <InsightCardSkeleton label={t('insights.tokens')} />
          <InsightCardSkeleton label={t('insights.cacheHit')} />
          <InsightCardSkeleton label={costMetricLabel} />
          <InsightCardSkeleton label={t('insights.topModel')} />
        </> : <>
          <InsightCard label={t('insights.tokens')} value={fmtTokens(totals.total_tokens)} detail={tf('insights.inputOutputDetail', fmtTokens(totals.input), fmtTokens(totals.output))} />
          <InsightCard label={t('insights.cacheHit')} value={fmtPercent(totals.cache_hit_rate)} detail={tf('insights.cacheDetail', fmtTokens(totals.cache_read), fmtTokens(totals.cache_write))} />
          <InsightCard label={costMetricLabel} value={fmtCost(totals.cost_usd)} detail={totals.unpriced_tokens ? tf('insights.unpricedApiCalls', fmtTokens(totals.unpriced_tokens), totals.api_calls || 0) : tf('insights.sessionsApiCalls', totals.sessions || 0, totals.api_calls || 0)} />
          <InsightCard label={t('insights.topModel')} value={topModel ? fmtTokens(topModel.periodTotals.total_tokens) : '—'} detail={topModel ? usageModelLabel(topModel) : t('insights.noUsage')} />
        </>}
      </div>
      <section className="insights-chart-card">
        <div className="insights-card-head"><div><h2>{tf('insights.byModel', usageMetricLabel(props.metric))}</h2><p>{tf('insights.recentTrend', periodLabel)}{coverageLabel ? ` · ${coverageLabel}` : ''}</p></div><button type="button" className="chart-stack-toggle icon-btn" aria-label={chartStacked ? t('insights.showUnstackedChart') : t('insights.showStackedChart')} title={chartStacked ? t('insights.showUnstackedChart') : t('insights.showStackedChart')} aria-pressed={chartStacked} onClick={() => setChartStacked((value) => !value)}>{chartStacked ? <LineChart /> : <Layers />}</button></div>
        {showSkeleton ? <UsageChartSkeleton /> : <><UsageAreaChart buckets={isSingleDay ? activeHours : activeDays} models={models} metric={props.metric} stacked={chartStacked} /><UsageShareBar models={models} metric={props.metric} /></>}
      </section>
      <div className="insights-grid">
        <section className="insights-panel"><div className="insights-panel-head"><h2>{t('insights.models')}</h2><div className="model-list-filters"><input type="search" list="insights-model-filter-options" className="model-filter-input" aria-label={t('insights.filterModels')} placeholder={t('insights.filterModels')} value={modelQuery} onChange={(event) => setModelQuery(event.target.value)} disabled={showSkeleton || !allModels.length} /><datalist id="insights-model-filter-options">{allModels.map((model) => { const label = usageModelLabel(model); return <option key={label} value={label} />; })}</datalist></div></div>{showSkeleton ? <ModelUsageSkeletonList /> : allModels.length ? filteredModels.length ? filteredModels.slice(0, 10).map((model, index) => <ModelUsageRow key={`${model.model}:${model.provider || 'unknown'}`} model={model} rank={index + 1} />) : <p className="insights-empty">{t('insights.noMatchingModels')}</p> : <p className="insights-empty">{t('insights.noWindowUsage')}</p>}</section>
        <section className="insights-panel"><h2>{t('insights.otherSignals')}</h2>{showSkeleton ? <SignalSkeletonList /> : <><SignalRow name={t('insights.reasoning')} value={fmtTokens(totals.reasoning)} /><SignalRow name={t('insights.tools')} value={`${totals.tool_calls || 0}`} /><SignalRow name={t('insights.avgSession')} value={fmtTokens(totals.avg_tokens_per_session)} /><SourceSignalList sources={activeSources.slice(0, 6)} /></>}</section>
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
  return <div className="usage-chart usage-chart-loading" aria-busy="true" aria-label={t('insights.loadingChart')}><div className="chart-loading-grid" aria-hidden="true">{[0, 1, 2, 3].map((item) => <span key={item} />)}</div><div className="chart-loading-line" aria-hidden="true" /><div className="chart-loading-line secondary" aria-hidden="true" /><div className="chart-loading-badge">{t('insights.loading')}</div></div>;
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
  return <div className="signal-row signal-row-sources"><span>{t('insights.sources')}</span><div className="source-channel-list">{sources.length ? sources.map((item) => <span className="source-channel-chip" key={item.source}><b>{item.source}</b><em>{fmtTokens(item.totals.total_tokens)}</em></span>) : <span className="source-channel-empty">—</span>}</div></div>;
}
function ModelUsageRow({ model, rank }: { model: UsageModel & { periodTotals: UsageTotals }; rank: number }) {
  const max = Math.max(1, model.periodTotals.total_tokens);
  const cache = Math.min(100, Math.round((model.periodTotals.cache_read / max) * 100));
  return <article className="model-usage-row"><div><b>#{rank}</b><span className="model-name" title={usageModelLabel(model)}><small className="model-provider">{model.provider || 'unknown'}</small>{model.model}</span></div><div className="model-value"><strong>{fmtTokens(model.periodTotals.total_tokens)}</strong><small className="model-cost-sub">{fmtMoney(model.periodTotals.cost_usd)}</small></div><p>{tf('insights.modelRowDetail', fmtTokens(model.periodTotals.input), fmtTokens(model.periodTotals.output), fmtTokens(model.periodTotals.cache_read), fmtPercent(model.periodTotals.cache_hit_rate))}</p><div className="model-bar"><i style={{ width: `${cache}%` }} /></div></article>;
}
function UsageShareBar({ models, metric }: { models: Array<UsageModel & { periodTotals: UsageTotals }>; metric: UsageMetric }) {
  const rawSlices = models.slice(0, 6).map((model, index) => ({ model: usageModelLabel(model), index, value: Number(model.periodTotals[metric] || 0) })).filter((item) => item.value > 0);
  const total = rawSlices.reduce((sum, item) => sum + item.value, 0);
  if (!rawSlices.length || total <= 0) return <div className="usage-share-chart"><p className="insights-empty">{t('insights.noMetricUsage')}</p></div>;
  let cursor = 0;
  const slices = rawSlices.map((item) => { const pct = (item.value / total) * 100; const start = cursor; cursor += pct; return { ...item, pct, start }; });
  return <div className="usage-share-chart" role="img" aria-label={tf('insights.modelShare', usageMetricLabel(metric))}>
    <div className="usage-share-map">
      <div className="usage-share-bar">{slices.map((item) => <span key={item.model} className="usage-share-segment" title={`${item.model} · ${formatMetricValue(metric, item.value)} · ${Math.round(item.pct)}%`} style={{ width: `${item.pct}%`, background: `var(--chart-${item.index})` }} />)}</div>
      <div className="usage-share-indicators">{slices.map((item) => { const pct = item.value / total; return <span key={item.model} className="usage-share-indicator" style={{ '--share-start': `${item.start}%`, '--share-width': `${Math.max(item.pct, 0.8)}%` } as React.CSSProperties}><i style={{ background: `var(--chart-${item.index})` }} /><b title={item.model}>{item.model}</b><em>{fmtPercent(pct)}</em></span>; })}</div>
    </div>
  </div>;
}
function UsageAreaChart({ buckets, models, metric, stacked }: { buckets: Array<UsageDay | UsageHour>; models: Array<UsageModel & { periodTotals: UsageTotals }>; metric: UsageMetric; stacked: boolean }) {
  const width = 720;
  const height = 260;
  const pad = { top: 14, right: 18, bottom: 28, left: 30 };
  const compactAxisLabels = useMediaQuery('(max-width: 760px)');
  const isHourly = isHourlyBucket(buckets[0]);
  const series = models.slice(0, 4).map((model, index) => ({ model: usageModelLabel(model), index, values: isHourlyBucket(buckets[0]) ? modelHourlyMetricValues(model, buckets as UsageHour[], metric) : modelDailyMetricValues(model, buckets as UsageDay[], metric) }));
  const totalValues = buckets.map((bucket) => metricValue(bucket, metric));
  const stackedSeries = series.reduce<Array<{ model: string; index: number; values: number[]; lower: number[]; upper: number[] }>>((acc, item) => {
    const lower = acc.length ? acc[acc.length - 1].upper : item.values.map(() => 0);
    const upper = item.values.map((value, index) => lower[index] + value);
    acc.push({ ...item, lower, upper });
    return acc;
  }, []);
  const allValues = [...totalValues, ...series.flatMap((item) => item.values), ...stackedSeries.flatMap((item) => item.upper)];
  const maxValue = Math.max(1, ...allValues);
  const yTicks = chartYAxisTicks(allValues, 4, (value) => metric === 'cost_usd' ? formatMetricValue(metric, value) : compactAxisLabels ? fmtCompactAxisTick(value) : formatMetricValue(metric, value));
  const pointSeries = stacked
    ? stackedSeries.map((item) => ({ model: item.model, index: item.index, values: item.values, pointValues: item.upper }))
    : series.map((item) => ({ model: item.model, index: item.index, values: item.values, pointValues: item.values }));
  const axisLabelVisible = (index: number) => buckets.length <= 7 || index === 0 || index === buckets.length - 1 || (isHourly && index % 6 === 0);
  const pointHitWidthPct = buckets.length > 1 ? (((width - pad.left - pad.right) / (buckets.length - 1)) * 0.8 / width) * 100 : (24 / width) * 100;
  return <div className={`usage-chart ${stacked ? 'stacked' : 'unstacked'}`} data-series-count={series.length}>
    <svg viewBox={`0 0 ${width} ${height}`} role="img" aria-label={t('insights.trendChart')} preserveAspectRatio="none">
      <defs>{series.map((item) => <linearGradient key={item.model} id={`insight-grad-${item.index}`} x1="0" x2="0" y1="0" y2="1"><stop offset="0%" stopColor={`var(--chart-${item.index})`} stopOpacity=".52" /><stop offset="100%" stopColor={`var(--chart-${item.index})`} stopOpacity=".06" /></linearGradient>)}</defs>
      <g className="chart-grid" aria-hidden="true">{yTicks.map((tick, tickIndex) => { const y = chartPoint(0, tick.value, 1, width, height, pad, maxValue).y; return <line key={`${tickIndex}-${tick.label}`} x1={pad.left} x2={width - pad.right} y1={y} y2={y} />; })}</g>
      {!stacked && <path className="usage-total-area" d={areaPath(totalValues, width, height, pad, maxValue)} />}
      {stacked ? stackedSeries.map((item) => <path key={item.model} className="usage-stack-area" d={stackedAreaPath(item.lower, item.upper, width, height, pad, maxValue)} fill={`url(#insight-grad-${item.index})`} />) : series.map((item) => <g key={item.model} className={`usage-series usage-series-${item.index}`}>
        <path className="usage-area" d={areaPath(item.values, width, height, pad, maxValue)} fill={`url(#insight-grad-${item.index})`} />
        <path className="usage-line" pathLength={1} d={linePath(item.values, width, height, pad, maxValue)} />
      </g>)}
      {stacked && <path className="usage-total-line" pathLength={1} d={linePath(totalValues, width, height, pad, maxValue)} />}
    </svg>
    <div className="chart-y-axis" aria-hidden="true">{yTicks.map((tick, tickIndex) => <span key={`${tickIndex}-${tick.label}`} style={{ top: `${tick.pct}%` }}>{tick.label}</span>)}</div>
    <div className="chart-points">{pointSeries.map((item) => item.values.map((value, pointIndex) => { const bucket = buckets[pointIndex]; const point = chartPoint(pointIndex, item.pointValues[pointIndex], item.values.length, width, height, pad, maxValue); const label = chartTooltipLabel(item.model, bucket?.label || '', value, usageMetricLabel(metric), formatMetricValue(metric, value)); const tooltipPlacement = chartTooltipPlacement(point.y, height); const tooltipAlign = chartTooltipAlignment(point.x, width); return <span key={`${item.model}-${isHourlyBucket(bucket) ? bucket.hour : bucket?.date || pointIndex}`} className={`chart-point-hit tooltip-${tooltipPlacement} tooltip-align-${tooltipAlign}`} tabIndex={0} aria-label={label} style={{ left: `${(point.x / width) * 100}%`, top: `${(point.y / height) * 100}%`, '--hit-width': `${pointHitWidthPct}%`, '--point-color': `var(--chart-${item.index})` } as React.CSSProperties}><span className="chart-tooltip" aria-hidden="true">{label}</span></span>; }))}</div>
    <div className="chart-axis">{buckets.map((bucket, index) => <span key={isHourlyBucket(bucket) ? bucket.hour : bucket.date} style={{ left: `${buckets.length === 1 ? 50 : (index / (buckets.length - 1)) * 100}%` }}>{axisLabelVisible(index) ? bucket.label : ''}</span>)}</div>
  </div>;
}
function ChatSidebar(props: { filter: string; setFilter: (v: string) => void; hideCronSessions: boolean; setHideCronSessions: (value: boolean) => void; startDraftSession: () => void; pinnedSessions: Session[]; normalSessions: Session[]; activeSessionId: string; setActiveSessionId: (v: string) => void; writeHashRoute: (route: HashRoute) => void; closeMobileSidebar: () => void; pinnedIds: Set<string>; togglePin: (id: string) => void; openSessionMenu: (session: Session, event: React.MouseEvent) => void; openSessionMenuAt: (session: Session, x: number, y: number) => void }) {
  const activateSession = (id: string) => {
    props.writeHashRoute({ mode: 'chat', sessionId: id });
    if (id === props.activeSessionId) {
      props.closeMobileSidebar();
      return;
    }
    props.setActiveSessionId(id);
    props.closeMobileSidebar();
  };
  return <><div className="session-searchbar"><button className="new-chat-btn" aria-label={t('chat.new')} title={t('chat.new')} onClick={() => { props.startDraftSession(); props.closeMobileSidebar(); }}><Plus /></button><input className="filter" placeholder={t('chat.search')} value={props.filter} onChange={(e) => props.setFilter(e.target.value)} /><button type="button" className={`session-filter-btn ${props.hideCronSessions ? 'active' : ''}`} aria-label={props.hideCronSessions ? t('chat.showCronSessions') : t('chat.hideCronSessions')} title={props.hideCronSessions ? t('chat.showCronSessions') : t('chat.hideCronSessions')} aria-pressed={props.hideCronSessions} onClick={() => props.setHideCronSessions(!props.hideCronSessions)}><SlidersHorizontal /></button></div><div className="sessions">{props.pinnedSessions.length > 0 && <div className="section-label"><ChevronRight /> {t('chat.pinned')}</div>}{props.pinnedSessions.map((s) => <SessionRow key={s.id} session={s} active={s.id === props.activeSessionId} pinned={props.pinnedIds.has(s.id)} onClick={() => activateSession(s.id)} onTogglePin={() => props.togglePin(s.id)} onContextMenu={(event) => props.openSessionMenu(s, event)} onLongPress={(x, y) => props.openSessionMenuAt(s, x, y)} />)}<div className="section-label"><ChevronRight /> {t('chat.recent')}</div>{props.normalSessions.map((s) => <SessionRow key={s.id} session={s} active={s.id === props.activeSessionId} pinned={props.pinnedIds.has(s.id)} onClick={() => activateSession(s.id)} onTogglePin={() => props.togglePin(s.id)} onContextMenu={(event) => props.openSessionMenu(s, event)} onLongPress={(x, y) => props.openSessionMenuAt(s, x, y)} />)}</div></>;
}
function SessionRow({ session, active, pinned, onClick, onTogglePin, onContextMenu, onLongPress }: { session: Session; active: boolean; pinned: boolean; onClick: () => void; onTogglePin: () => void; onContextMenu: (event: React.MouseEvent) => void; onLongPress: (x: number, y: number) => void }) {
  const longPress = useLongPressContextMenu(onLongPress);
  const leadingIcon = session.source === 'cron' ? <CalendarClock /> : session.source === 'cli' ? <Terminal /> : session.source === 'alp-worker' ? <Bot /> : pinned ? <Star /> : null;
  return <div className={`session-item ${active ? 'active' : ''} ${pinned ? 'pinned' : ''} ${leadingIcon ? 'has-leading-icon' : ''}`} role="button" tabIndex={0} onClick={onClick} onContextMenu={onContextMenu} onPointerDown={longPress.onPointerDown} onPointerMove={longPress.onPointerMove} onPointerUp={longPress.onPointerUp} onPointerCancel={longPress.onPointerCancel} onKeyDown={(e) => { if (e.key === 'Enter') onClick(); }}>{leadingIcon && <span className="session-icon">{leadingIcon}</span>}<span className="session-text"><span className="session-title">{sessionDisplayTitle(session)}</span><span className="session-preview">{compactSessionPreview(session.preview || `${session.message_count || 0} messages`)}</span></span><button type="button" className="pin-hit" onClick={(e) => { e.stopPropagation(); onTogglePin(); }} title={pinned ? t('chat.unpin') : t('chat.pin')}>{pinned ? <PinOff /> : <Pin />}</button></div>;
}
function ChatImageLightbox({ items, current, onSelect, onClose }: { items: ChatLightboxImage[]; current: ChatLightboxImage | null; onSelect: (item: ChatLightboxImage) => void; onClose: () => void }) {
  const imgRef = useRef<HTMLImageElement | null>(null);
  const zoom = useRef({ scale: 1, x: 0, y: 0 });
  const pan = useRef<{ id: number; x: number; y: number; panX: number; panY: number } | null>(null);
  const chatPointers = useRef(new Map<number, { x: number; y: number }>());
  const chatPinchStart = useRef<{ distance: number; center: { x: number; y: number }; scale: number; x: number; y: number; imageCenter: { x: number; y: number } } | null>(null);
  const [metadataOpen, setMetadataOpen] = useState(false);
  const [dimensions, setDimensions] = useState<{ width: number; height: number } | null>(null);
  const index = current ? items.findIndex((item) => item.key === current.key) : -1;
  const applyZoom = (transition = false) => {
    const img = imgRef.current;
    if (!img) return;
    if (zoom.current.scale <= 1.01) {
      zoom.current = { scale: 1, x: 0, y: 0 };
      img.classList.remove('zoomed', 'panning');
      img.style.transform = '';
      img.style.transition = transition ? 'transform 160ms cubic-bezier(.2,.8,.2,1)' : 'none';
      return;
    }
    const extraX = Math.max(0, img.offsetWidth * (zoom.current.scale - 1) / 2 + 96);
    const extraY = Math.max(0, img.offsetHeight * (zoom.current.scale - 1) / 2 + 96);
    zoom.current.x = clampNumber(zoom.current.x, -extraX, extraX);
    zoom.current.y = clampNumber(zoom.current.y, -extraY, extraY);
    img.classList.add('zoomed');
    img.style.transition = transition ? 'transform 160ms cubic-bezier(.2,.8,.2,1)' : 'none';
    img.style.transform = `translate3d(${zoom.current.x}px, ${zoom.current.y}px, 0) scale(${zoom.current.scale})`;
  };
  const resetZoom = () => { zoom.current = { scale: 1, x: 0, y: 0 }; pan.current = null; chatPinchStart.current = null; chatPointers.current.clear(); applyZoom(); };
  const selectRelative = (dir: -1 | 1) => {
    if (index < 0) return;
    const next = items[index + dir];
    if (next) onSelect(next);
  };
  const downloadCurrent = () => {
    if (!current) return;
    const a = document.createElement('a');
    a.href = current.downloadUrl;
    a.download = current.name;
    document.body.appendChild(a);
    a.click();
    a.remove();
  };
  const onWheel = (event: React.WheelEvent<HTMLDivElement>) => {
    if (!current) return;
    if ((event.target as HTMLElement).closest('.modalbar,.modal-meta,a,button')) return;
    event.preventDefault();
    event.stopPropagation();
    const img = imgRef.current;
    if (!img) return;
    const oldScale = zoom.current.scale;
    const nextScale = clampNumber(oldScale * Math.exp(-event.deltaY * 0.0018), 1, 6);
    const rect = img.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const ratio = nextScale / oldScale;
    zoom.current.x -= (event.clientX - cx) * (ratio - 1);
    zoom.current.y -= (event.clientY - cy) * (ratio - 1);
    zoom.current.scale = nextScale;
    applyZoom();
  };
  const chatPointerList = () => Array.from(chatPointers.current.values());
  const chatPointerDistance = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.hypot(a.x - b.x, a.y - b.y);
  const chatPointerCenter = (a: { x: number; y: number }, b: { x: number; y: number }) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
  const beginChatPinch = () => {
    const pts = chatPointerList();
    const img = imgRef.current;
    if (pts.length < 2 || !img) return;
    const center = chatPointerCenter(pts[0], pts[1]);
    const rect = img.getBoundingClientRect();
    chatPinchStart.current = { distance: Math.max(1, chatPointerDistance(pts[0], pts[1])), center, scale: zoom.current.scale, x: zoom.current.x, y: zoom.current.y, imageCenter: { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 } };
    pan.current = null;
    img.classList.add('panning');
    img.style.transition = 'none';
  };
  const beginChatPan = (event: React.PointerEvent<HTMLDivElement>) => {
    pan.current = { id: event.pointerId, x: event.clientX, y: event.clientY, panX: zoom.current.x, panY: zoom.current.y };
    imgRef.current?.classList.add('panning');
    if (imgRef.current) imgRef.current.style.transition = 'none';
    try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* ignore */ }
  };
  const onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!current) return;
    if ((event.target as HTMLElement).closest('.modalbar,.modal-meta,a,button')) return;
    if (event.pointerType === 'touch') {
      chatPointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
      try { event.currentTarget.setPointerCapture(event.pointerId); } catch { /* ignore */ }
      if (chatPointers.current.size >= 2) { event.preventDefault(); beginChatPinch(); return; }
      if (zoom.current.scale > 1.01) { event.preventDefault(); beginChatPan(event); }
      return;
    }
    if (event.button !== 0 || zoom.current.scale <= 1.01) return;
    event.preventDefault();
    beginChatPan(event);
  };
  const onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'touch' && chatPointers.current.has(event.pointerId)) chatPointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (chatPinchStart.current && event.pointerType === 'touch' && chatPointers.current.size >= 2) {
      event.preventDefault();
      const pts = chatPointerList();
      const center = chatPointerCenter(pts[0], pts[1]);
      const distance = Math.max(1, chatPointerDistance(pts[0], pts[1]));
      const start = chatPinchStart.current;
      zoom.current.scale = clampNumber(start.scale * distance / start.distance, 1, 6);
      const ratio = zoom.current.scale / start.scale;
      zoom.current.x = start.x + center.x - start.center.x - (start.center.x - start.imageCenter.x) * (ratio - 1);
      zoom.current.y = start.y + center.y - start.center.y - (start.center.y - start.imageCenter.y) * (ratio - 1);
      if (zoom.current.scale <= 1.01) zoom.current = { scale: 1, x: 0, y: 0 };
      applyZoom();
      return;
    }
    if (!pan.current || pan.current.id !== event.pointerId) return;
    event.preventDefault();
    zoom.current.x = pan.current.panX + event.clientX - pan.current.x;
    zoom.current.y = pan.current.panY + event.clientY - pan.current.y;
    applyZoom();
  };
  const endPointer = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'touch') chatPointers.current.delete(event.pointerId);
    if (chatPinchStart.current && chatPointers.current.size < 2) {
      chatPinchStart.current = null;
      imgRef.current?.classList.remove('panning');
      if (zoom.current.scale <= 1.01) resetZoom();
      event.preventDefault();
      return;
    }
    if (!pan.current || pan.current.id !== event.pointerId) return;
    pan.current = null;
    imgRef.current?.classList.remove('panning');
  };
  const cancelPointer = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.pointerType === 'touch') chatPointers.current.delete(event.pointerId);
    chatPinchStart.current = null;
    pan.current = null;
    imgRef.current?.classList.remove('panning');
  };
  useEffect(() => { resetZoom(); setDimensions(null); setMetadataOpen(false); }, [current?.key]);
  useEffect(() => {
    if (!current) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (event.key === 'Escape') onClose();
      if (event.key === 'ArrowLeft') { event.preventDefault(); selectRelative(-1); }
      if (event.key === 'ArrowRight') { event.preventDefault(); selectRelative(1); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [current?.key, index, items]);
  if (!current) return null;
  return <div className={`image-modal chat-image-modal ${metadataOpen ? 'metadata-open' : ''}`} onClick={(event) => { if (event.target === event.currentTarget) onClose(); }} onWheel={onWheel} onPointerDown={onPointerDown} onPointerMove={onPointerMove} onPointerUp={endPointer} onPointerCancel={cancelPointer}>
    <img ref={imgRef} className="image-modal-img" src={current.src} alt={current.name} onLoad={(event) => { setDimensions({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight }); applyZoom(); }} onClick={(event) => event.stopPropagation()} />
    <aside className="modal-meta" onClick={(event) => event.stopPropagation()}>
      <h2>{t('gallery.metadata')}</h2>
      <p className="metadata-dim">{t('gallery.dimensions')}: {dimensions ? `${dimensions.width} × ${dimensions.height}` : '—'}</p>
      <section className="metadata-files-section"><span>{t('gallery.files')}</span><p>{current.name}</p><p>{tf('gallery.messageId', current.messageId)}</p></section>
      <section className="metadata-png-section"><span>{t('gallery.source')}</span><p>{current.path}</p></section>
    </aside>
    <div className="modalbar" onClick={(event) => event.stopPropagation()}>
      <button className="mobile-icon-only" aria-label={t('gallery.download')} onClick={downloadCurrent}><Download /></button>
      <button className={`mobile-icon-only modal-metadata-toggle ${metadataOpen ? 'active' : ''}`} aria-label={t('gallery.metadata')} aria-expanded={metadataOpen} onClick={() => setMetadataOpen((value) => !value)}><Info /></button>
      <button className="mobile-icon-only" aria-label={t('gallery.previous')} disabled={index <= 0} onClick={() => selectRelative(-1)}><ChevronLeft /></button>
      <button className="mobile-icon-only" aria-label={t('gallery.next')} disabled={index < 0 || index >= items.length - 1} onClick={() => selectRelative(1)}><ChevronRight /></button>
      <button className="mobile-icon-only" aria-label={t('gallery.close')} onClick={onClose}><X /></button>
    </div>
  </div>;
}
function DropdownControl({ icon, ariaLabel, label = '', value, valueProvider = '', options, onChange, wide = false, hideLabel = false, searchable = false, placement = 'up', iconOnly = false, className = '', emptyLabel = t('chat.noModels') }: { icon: React.ReactNode; ariaLabel: string; label?: string; value: string; valueProvider?: string; options: Array<{ id: string; label: string; provider?: string; description?: string }>; onChange: (value: string, option?: { id: string; label: string; provider?: string; description?: string }) => void; wide?: boolean; hideLabel?: boolean; searchable?: boolean; placement?: 'up' | 'down'; iconOnly?: boolean; className?: string; emptyLabel?: string }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const rootRef = useRef<HTMLDivElement>(null);
  const current = findModelOption(options, value, valueProvider) || { id: value, label: value, provider: valueProvider || undefined };
  const currentKey = modelOptionKey(current);
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
  return <div ref={rootRef} className={`dropdown-control ${wide ? 'wide' : ''} ${searchable ? 'searchable' : ''} ${placement === 'down' ? 'drop-down' : 'drop-up'} ${iconOnly ? 'icon-only' : ''} ${open ? 'open' : ''} ${className}`.trim()}>
    <button type="button" className="dropdown-trigger" aria-label={ariaLabel} title={ariaLabel} aria-expanded={open} onClick={() => { setOpen((v) => !v); setQuery(''); }}>
      <span className="dropdown-icon">{icon}</span>
      {!iconOnly && <span className="dropdown-copy">{!hideLabel && <span className="dropdown-label">{label || ariaLabel}</span>}<span className="dropdown-value">{current.label}</span></span>}
      {!iconOnly && <ChevronRight className="dropdown-caret" />}
    </button>
    {open && <div className="dropdown-menu" role="listbox">
      {searchable && <input className="dropdown-search" autoFocus placeholder={t('chat.searchModels')} value={query} onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.stopPropagation()} />}
      {filteredOptions.map((item) => <button type="button" role="option" aria-selected={modelOptionKey(item) === currentKey} className={modelOptionKey(item) === currentKey ? 'selected' : ''} key={modelOptionKey(item)} onClick={() => { onChange(item.id, item); setOpen(false); }}><span className="dropdown-option-copy"><span className="dropdown-option-label">{item.label}</span>{item.description && <span className="dropdown-option-description">{item.description}</span>}</span></button>)}
      {filteredOptions.length === 0 && <span className="dropdown-empty">{emptyLabel}</span>}
    </div>}
  </div>;
}

function formatNavigatorTime(value?: string | number) {
  if (value === undefined || value === null || value === '') return '';
  const raw = Number(value);
  const date = Number.isFinite(raw) ? new Date(raw * (raw < 1e12 ? 1000 : 1)) : new Date(String(value));
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleString([], { month: 'short', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}

function sameStringSet(a: Set<string>, b: Set<string>) {
  if (a.size !== b.size) return false;
  for (const item of a) if (!b.has(item)) return false;
  return true;
}

function numericMessageId(value?: string | number | null): number | null {
  const text = String(value || '');
  if (!/^\d+$/.test(text)) return null;
  const parsed = Number(text);
  return Number.isFinite(parsed) ? parsed : null;
}

function subagentBeforeTimeForVisibleRange(scroller: HTMLElement | null, messages: ChatMessage[], hasNewer: boolean): number | null | undefined {
  if (!scroller) return null;
  if (subagentViewportIsLive(scroller, hasNewer)) return undefined;
  const viewport = scroller.getBoundingClientRect();
  const rows = Array.from(scroller.querySelectorAll<HTMLElement>('[data-message-id]'));
  const visibleIds = new Set(rows
    .filter((row) => {
      const rect = row.getBoundingClientRect();
      return rect.bottom >= viewport.top + 4 && rect.top <= viewport.bottom - 4;
    })
    .map((row) => row.getAttribute('data-message-id') || '')
    .filter(Boolean));
  const visibleTime = subagentBeforeTimeForMessages(messages, visibleIds);
  if (visibleTime !== undefined) return visibleTime;
  const nearestRowIds = subagentPrecedingFallbackIds(rows.map((row) => {
    const rect = row.getBoundingClientRect();
    return { id: row.getAttribute('data-message-id') || '', top: rect.top, bottom: rect.bottom, rendered: rect.width > 0 || rect.height > 0 };
  }), viewport.top);
  for (const rowId of nearestRowIds) {
    const nearestTime = subagentBeforeTimeForMessages(messages, new Set([rowId]));
    if (nearestTime !== undefined) return nearestTime;
  }
  return null;
}

function activeNavigatorIdsForVisibleRange(scroller: HTMLElement | null, items: UserMessageNavItem[]): Set<string> {
  const active = new Set<string>();
  if (!scroller || !items.length) return active;
  const viewport = scroller.getBoundingClientRect();
  const rows = Array.from(scroller.querySelectorAll<HTMLElement>('.msg-row[data-message-id]')).filter((row) => {
    const rect = row.getBoundingClientRect();
    return rect.bottom >= viewport.top + 4 && rect.top <= viewport.bottom - 4;
  });
  if (!rows.length) return active;
  const visibleIds = new Set(rows.map((row) => row.getAttribute('data-message-id') || '').filter(Boolean));
  for (const item of items) if (visibleIds.has(item.id)) active.add(item.id);
  const visibleNumbers = rows.map((row) => numericMessageId(row.getAttribute('data-message-id'))).filter((value): value is number => value !== null);
  if (!visibleNumbers.length) return active;
  const start = Math.min(...visibleNumbers);
  const end = Math.max(...visibleNumbers);
  const numericItems = items.map((item) => ({ item, numeric: numericMessageId(item.id) })).filter((entry): entry is { item: UserMessageNavItem; numeric: number } => entry.numeric !== null);
  numericItems.forEach((entry, index) => {
    const itemNumeric = entry.numeric;
    const nextNumeric = numericItems[index + 1]?.numeric ?? Infinity;
    if (itemNumeric <= end && nextNumeric > start) active.add(entry.item.id);
  });
  return active;
}

function ChatUserNavigator({ items, loading, sessionId, activeIds, onJumpToMessage, chatScrollRef }: { items: UserMessageNavItem[]; loading: boolean; sessionId: string; activeIds: Set<string>; onJumpToMessage: (sessionId: string, messageId: string) => void | Promise<void>; chatScrollRef: React.RefObject<HTMLElement | null> }) {
  const navRef = useRef<HTMLElement | null>(null);
  const trackRef = useRef<HTMLDivElement | null>(null);
  const popupTimerRef = useRef<number | null>(null);
  const initialMinimapScrollSessionRef = useRef('');
  const isMobileNavigator = useMediaQuery('(max-width: 760px)');
  const [popup, setPopup] = useState<{ item: UserMessageNavItem; top: number } | null>(null);
  const [scrollFade, setScrollFade] = useState({ before: false, after: false });
  const updateNavigatorMetrics = useCallback(() => {
    const nav = navRef.current;
    const track = trackRef.current;
    const scroller = chatScrollRef.current;
    if (nav && scroller) nav.style.setProperty('--user-minimap-max-height', `${Math.floor(scroller.clientHeight * 0.75)}px`);
    if (!track) return;
    setScrollFade({
      before: track.scrollTop > 1,
      after: track.scrollTop + track.clientHeight < track.scrollHeight - 1,
    });
  }, [chatScrollRef]);
  const clearPopupTimer = useCallback(() => {
    if (popupTimerRef.current === null) return;
    window.clearTimeout(popupTimerRef.current);
    popupTimerRef.current = null;
  }, []);
  const hidePopup = useCallback(() => { clearPopupTimer(); setPopup(null); }, [clearPopupTimer]);
  const showPopup = useCallback((item: UserMessageNavItem, target: HTMLElement, autoHide = false) => {
    clearPopupTimer();
    const navRect = navRef.current?.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    setPopup({ item, top: targetRect.top + targetRect.height / 2 - (navRect?.top ?? 0) });
    if (autoHide) popupTimerRef.current = window.setTimeout(() => setPopup(null), 3000);
  }, [clearPopupTimer]);
  useLayoutEffect(() => {
    updateNavigatorMetrics();
    const scroller = chatScrollRef.current;
    const track = trackRef.current;
    const observer = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(updateNavigatorMetrics) : null;
    if (scroller) observer?.observe(scroller);
    if (track) observer?.observe(track);
    window.addEventListener('resize', updateNavigatorMetrics);
    return () => {
      observer?.disconnect();
      window.removeEventListener('resize', updateNavigatorMetrics);
    };
  }, [chatScrollRef, items.length, updateNavigatorMetrics]);
  useLayoutEffect(() => {
    const track = trackRef.current;
    if (!track || !items.length || initialMinimapScrollSessionRef.current === sessionId) return;
    initialMinimapScrollSessionRef.current = sessionId;
    const scrollBottom = () => {
      if (initialMinimapScrollSessionRef.current !== sessionId) return;
      track.scrollTop = track.scrollHeight;
      updateNavigatorMetrics();
    };
    scrollBottom();
    const frame = window.requestAnimationFrame(scrollBottom);
    return () => window.cancelAnimationFrame(frame);
  }, [items.length, sessionId, updateNavigatorMetrics]);
  useEffect(() => () => clearPopupTimer(), [clearPopupTimer]);
  useEffect(() => {
    if (!isMobileNavigator || !popup) return;
    const closeMobilePopupOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target as Node | null;
      if (target && navRef.current?.contains(target)) return;
      hidePopup();
    };
    document.addEventListener('pointerdown', closeMobilePopupOnOutsidePointer);
    return () => document.removeEventListener('pointerdown', closeMobilePopupOnOutsidePointer);
  }, [hidePopup, isMobileNavigator, popup]);
  const handlePopupClick = useCallback((item: UserMessageNavItem) => {
    hidePopup();
    onJumpToMessage(sessionId, item.id);
  }, [hidePopup, onJumpToMessage, sessionId]);
  const handleNavigatorClick = useCallback((item: UserMessageNavItem) => {
    hidePopup();
    onJumpToMessage(sessionId, item.id);
  }, [hidePopup, onJumpToMessage, sessionId]);
  if (!sessionId || sessionId === DRAFT_SESSION_ID) return null;
  return <nav ref={navRef} className={`chat-user-minimap${loading ? ' loading' : ''}`} aria-label={t('chat.userNavigator')} aria-busy={loading} onMouseLeave={() => { if (!isMobileNavigator) hidePopup(); }}>
    <div ref={trackRef} className={`user-minimap-track${scrollFade.before ? ' can-scroll-before' : ''}${scrollFade.after ? ' can-scroll-after' : ''}`} onScroll={updateNavigatorMetrics}>
      {loading
        ? <div className="user-minimap-placeholder" aria-hidden="true">{Array.from({ length: 9 }, (_, index) => <span key={index} />)}</div>
        : items.map((item) => <button type="button" className={`user-minimap-hit${activeIds.has(item.id) ? ' active' : ''}`} key={item.id} aria-label={item.content} data-nav-index={item.index} data-nav-total={item.total} onPointerEnter={(event) => { if (!isMobileNavigator) showPopup(item, event.currentTarget); }} onFocus={(event) => showPopup(item, event.currentTarget)} onBlur={() => { if (!isMobileNavigator) hidePopup(); }} onClick={(event) => { if (isMobileNavigator) showPopup(item, event.currentTarget); else handleNavigatorClick(item); }}>
          <span className="user-minimap-bar" />
        </button>)}
    </div>
    {popup && <button type="button" className="user-minimap-popup" style={{ top: `${popup.top}px` }} onClick={() => handlePopupClick(popup.item)}><strong>{popup.item.content || 'User message'}</strong>{popup.item.assistant_preview && <span className="user-minimap-assistant-preview">{popup.item.assistant_preview}</span>}{formatNavigatorTime(popup.item.timestamp) && <time>{formatNavigatorTime(popup.item.timestamp)}</time>}</button>}
  </nav>;
}

type ChatMainProps = {
  sessions: Session[];
  activeSessionDetail: Session | null;
  activeSessionModelOverride?: SessionModelOverride;
  activeSessionId: string;
  messages: ChatMessage[];
  userMessageNav: UserMessageNavItem[];
  userNavLoading: boolean;
  historyTotal: number | null;
  onJumpToMessage: (sessionId: string, messageId: string) => void | Promise<void>;
  contextWindowSnapshot: ContextWindowSnapshot | null;
  showReasoning: boolean;
  setShowReasoning: React.Dispatch<React.SetStateAction<boolean>>;
  showToolCalls: boolean;
  setShowToolCalls: React.Dispatch<React.SetStateAction<boolean>>;
  desktopCompactMessages: boolean;
  setDesktopCompactMessages: React.Dispatch<React.SetStateAction<boolean>>;
  hasOlder: boolean;
  hasNewer: boolean;
  loadingMessages: boolean;
  loadMessageWindow: (sessionId: string, direction: 'latest' | 'older' | 'newer') => Promise<void>;
  attachments: Attachment[];
  setAttachments: React.Dispatch<React.SetStateAction<Attachment[]>>;
  input: string;
  setInput: React.Dispatch<React.SetStateAction<string>>;
  onFiles: (files: FileList | null) => Promise<void>;
  fileInput: React.RefObject<HTMLInputElement | null>;
  sendMessage: () => Promise<void>;
  stopStreaming: () => void;
  composerEnterMode: ComposerEnterMode;
  model: string;
  selectedModelProvider: string;
  setModel: (model: string, option?: ModelOption) => void;
  models: ModelOption[];
  effort: (typeof EFFORTS)[number];
  setEffort: React.Dispatch<React.SetStateAction<(typeof EFFORTS)[number]>>;
  busy: boolean;
  streaming: boolean;
  followUpQueue: FollowUpQueueItem[];
  onSteerQueuedItem: (item: FollowUpQueueItem) => Promise<void>;
  onEditQueuedItem: (item: FollowUpQueueItem) => void;
  onReorderQueuedItem: (fromIndex: number, toIndex: number) => void;
  chatScrollRef: React.RefObject<HTMLElement | null>;
  composerRef: React.RefObject<HTMLElement | null>;
  composerCompact: boolean;
  setComposerCompact: React.Dispatch<React.SetStateAction<boolean>>;
  theme: Theme;
  setTheme: React.Dispatch<React.SetStateAction<Theme>>;
  mobileSidebarOpen: boolean;
  toggleMobileSidebar: () => void;
  mode: Mode;
  onNavigateToSettings: () => void;
  newMessageCount: number;
  newMessageBoundaryId: string;
  onClearNewMessages: () => void;
};

function ChatMain(props: ChatMainProps) {
  const active = props.sessions.find((s: Session) => s.id === props.activeSessionId) || props.activeSessionDetail;
  const isMobile = useMediaQuery('(max-width: 760px)');
  const isCompactViewport = useMediaQuery('(max-width: 760px), (min-width: 761px) and (max-width: 1180px) and (orientation: landscape) and (max-height: 820px)');
  const isSmallLandscape = useMediaQuery('(min-width: 761px) and (max-width: 1180px) and (orientation: landscape) and (max-height: 820px)');
  const [isFullscreen, setIsFullscreen] = useState(false);
  useEffect(() => {
    const syncFullscreenState = () => setIsFullscreen(!!document.fullscreenElement);
    syncFullscreenState();
    document.addEventListener('fullscreenchange', syncFullscreenState);
    return () => document.removeEventListener('fullscreenchange', syncFullscreenState);
  }, []);
  const toggleFullscreen = useCallback(async () => {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else if (document.documentElement.requestFullscreen) await document.documentElement.requestFullscreen();
    } catch {
      // Fullscreen can be denied by the browser or platform; keep the control state unchanged.
    }
  }, []);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const resizeComposerTextarea = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    if (props.composerCompact) {
      textarea.style.height = '';
      textarea.style.maxHeight = '';
      textarea.style.overflowY = '';
      textarea.style.removeProperty('--composer-textarea-pad-bottom');
      return;
    }
    const minHeight = 48;
    const maxHeight = Math.max(minHeight, Math.floor(visibleViewportHeight(window) * 0.2));
    textarea.style.removeProperty('--composer-textarea-pad-bottom');
    textarea.style.height = 'auto';
    textarea.style.maxHeight = `${maxHeight}px`;
    textarea.style.overflowY = 'hidden';
    const basePaddingBottom = parseFloat(getComputedStyle(textarea).paddingBottom || '0') || 0;
    const footer = props.composerRef.current?.querySelector('.composer-footer') as HTMLElement | null;
    const needsInternalScroll = textarea.scrollHeight > maxHeight;
    if (needsInternalScroll && footer) {
      textarea.style.setProperty('--composer-textarea-pad-bottom', `${Math.ceil(basePaddingBottom + footer.offsetHeight + 8)}px`);
      textarea.style.height = 'auto';
    }
    const contentHeight = textarea.value.trim() ? textarea.scrollHeight : minHeight;
    const nextHeight = Math.min(Math.max(contentHeight, minHeight), maxHeight);
    textarea.style.height = `${nextHeight}px`;
    textarea.style.overflowY = textarea.scrollHeight > maxHeight ? 'auto' : 'hidden';
  }, [isMobile, props.composerCompact]);
  useLayoutEffect(() => { resizeComposerTextarea(); }, [props.input, props.composerCompact, resizeComposerTextarea]);
  useEffect(() => {
    window.addEventListener('resize', resizeComposerTextarea);
    return () => window.removeEventListener('resize', resizeComposerTextarea);
  }, [resizeComposerTextarea]);
  const collapseComposerForHistory = () => {
    if (!isCompactViewport) return;
    const activeElement = document.activeElement;
    if (activeElement instanceof HTMLElement && props.composerRef.current?.contains(activeElement)) activeElement.blur();
    props.setComposerCompact(true);
  };
  const [showLatestButton, setShowLatestButton] = useState(() => props.hasNewer);
  const latestButtonVisible = props.hasNewer || showLatestButton;
  const [latestDetailExpansion, setLatestDetailExpansion] = useState({ sessionId: '', token: 0 });
  const forceOpenLatestDetailToken = latestDetailExpansion.sessionId === props.activeSessionId ? latestDetailExpansion.token : 0;
  const updateLatestButton = useCallback(() => {
    setShowLatestButton(chatLatestButtonVisible(props.chatScrollRef.current, props.hasNewer));
  }, [props.chatScrollRef, props.hasNewer]);
  const scrollLatestViewport = useCallback(() => {
    const scrollBottom = () => {
      const scroller = props.chatScrollRef.current;
      if (scroller) scroller.scrollTop = scroller.scrollHeight;
    };
    scrollBottom();
    window.requestAnimationFrame(scrollBottom);
    window.setTimeout(scrollBottom, 60);
    window.setTimeout(scrollBottom, 300);
    window.setTimeout(scrollBottom, 800);
  }, [props.chatScrollRef]);
  const jumpToLatest = useCallback(async () => {
    if (props.hasNewer) await props.loadMessageWindow(props.activeSessionId, 'latest');
    props.onClearNewMessages();
    setLatestDetailExpansion((current) => ({
      sessionId: props.activeSessionId,
      token: current.sessionId === props.activeSessionId ? current.token + 1 : 1,
    }));
    setShowLatestButton(false);
    scrollLatestViewport();
  }, [props.activeSessionId, props.hasNewer, props.loadMessageWindow, props.onClearNewMessages, scrollLatestViewport]);
  useLayoutEffect(() => {
    const frame = window.requestAnimationFrame(updateLatestButton);
    return () => window.cancelAnimationFrame(frame);
  }, [props.activeSessionId, props.hasNewer, props.messages.length, props.messages.at(-1)?.id, updateLatestButton]);
  const [subagentWindow, setSubagentWindow] = useState<{ sessionId: string; beforeTime: number | null | undefined }>(() => ({ sessionId: props.activeSessionId, beforeTime: null }));
  const updateSubagentWindow = useCallback(() => {
    const beforeTime = subagentBeforeTimeForVisibleRange(props.chatScrollRef.current, props.messages, props.hasNewer);
    setSubagentWindow((current) => current.sessionId === props.activeSessionId && current.beforeTime === beforeTime
      ? current
      : { sessionId: props.activeSessionId, beforeTime });
  }, [props.activeSessionId, props.chatScrollRef, props.hasNewer, props.messages]);
  const subagentWindowTimerRef = useRef<number | null>(null);
  const scheduleSubagentWindowUpdate = useCallback(() => {
    if (subagentWindowTimerRef.current !== null) window.clearTimeout(subagentWindowTimerRef.current);
    subagentWindowTimerRef.current = window.setTimeout(() => {
      subagentWindowTimerRef.current = null;
      updateSubagentWindow();
    }, 150);
  }, [updateSubagentWindow]);
  useEffect(() => () => {
    if (subagentWindowTimerRef.current !== null) window.clearTimeout(subagentWindowTimerRef.current);
  }, []);
  const subagentBeforeTime = subagentWindow.sessionId === props.activeSessionId ? subagentWindow.beforeTime : null;
  const onScroll = (e: React.UIEvent<HTMLElement>) => {
    const el = e.currentTarget;
    if (isCompactViewport && !props.composerRef.current?.contains(document.activeElement)) props.setComposerCompact(true);
    if (shouldLoadOlderFromScroll(el, props.hasOlder, props.loadingMessages)) props.loadMessageWindow(props.activeSessionId, 'older');
    if (shouldLoadNewerFromScroll(el, props.hasNewer, props.loadingMessages)) props.loadMessageWindow(props.activeSessionId, 'newer');
    setShowLatestButton(chatLatestButtonVisible(el, props.hasNewer));
    updateActiveNavigatorIds();
    scheduleSubagentWindowUpdate();
    if (el.scrollHeight - el.scrollTop - el.clientHeight < 120 && props.newMessageCount > 0) props.onClearNewMessages();
  };
  const onWheel = (e: React.WheelEvent<HTMLElement>) => {
    collapseComposerForHistory();
    if (shouldLoadOlderFromWheel(e.currentTarget, e.deltaY, props.hasOlder, props.loadingMessages)) props.loadMessageWindow(props.activeSessionId, 'older');
  };
  const sessionModelOverride = props.activeSessionModelOverride;
  const sessionModel = sessionModelOverride?.model || realModelOrEmpty(active?.model) || realModelOrEmpty(props.activeSessionDetail?.model) || realModelOrEmpty(props.model) || props.models[0]?.id || '';
  const messageProvider = latestMessageProviderForModel(props.messages, sessionModel);
  const apiSessionProvider = String(active?.provider || props.activeSessionDetail?.provider || messageProvider).trim();
  const sessionProvider = String(sessionModelOverride?.provider ?? (props.activeSessionId === DRAFT_SESSION_ID ? props.selectedModelProvider : apiSessionProvider)).trim();
  const currentModel = sessionModel;
  const activeTitle = active?.id === DRAFT_SESSION_ID ? 'New conversation' : active ? sessionDisplayTitle(active) : 'Hermes Agent';
  const headerTimes = sessionHeaderTimes(active, props.messages);
  const [activeNavigatorIds, setActiveNavigatorIds] = useState<Set<string>>(() => new Set());
  const updateActiveNavigatorIds = useCallback(() => {
    const next = activeNavigatorIdsForVisibleRange(props.chatScrollRef.current, props.userMessageNav || []);
    setActiveNavigatorIds((old) => sameStringSet(old, next) ? old : next);
  }, [props.chatScrollRef, props.userMessageNav]);
  const exactCurrentOption = currentModel ? findModelOption(props.models, currentModel, sessionProvider) : undefined;
  const currentOption = currentModel && !exactCurrentOption ? currentModelDisplayOption(currentModel, props.models, sessionProvider) : undefined;
  const currentModelOption = exactCurrentOption || currentOption;
  const contextModelOption = exactCurrentOption || (currentModel ? findModelOption(props.models, currentModel) : undefined);
  const modelOptions = currentOption ? [currentOption, ...props.models] : props.models;
  const effortOptions = EFFORTS.map((x) => ({ id: x, label: x }));
  const visibleMessages = useMemo(() => visibleChatMessages<ChatMessage>(props.messages, props.showReasoning, props.showToolCalls), [props.messages, props.showReasoning, props.showToolCalls]);
  const primaryActionIsStop = props.streaming && !props.input.trim();
  const loadTranscriptTurnDetails = useCallback(async (detail: TurnDetailMetadata): Promise<ChatMessage[]> => {
    if (!props.activeSessionId || !detail.beforeId) return [];
    const detailParams = new URLSearchParams({ limit: String(MESSAGE_PAGE * 4) });
    detailParams.set('view', 'details');
    if (detail.afterId) detailParams.set('after', detail.afterId);
    detailParams.set('before', detail.beforeId);
    const response = await fetch(`/chat/messages/${encodeURIComponent(props.activeSessionId)}?${detailParams}`);
    if (!response.ok) throw new Error(await response.text());
    const page: MessagePage = await response.json();
    return normalizeChatHistoryChunk<ChatMessage>(page.data || [], (raw) => normalizeMessage(raw, active?.source));
  }, [active?.source, props.activeSessionId]);
  const [chatImageModal, setChatImageModal] = useState<ChatLightboxImage | null>(null);
  const chatLightboxImages = useMemo(() => visibleMessages.flatMap((message: ChatMessage) => chatMediaImagesFromMarkdown(message.content || '').map((image, index) => ({ ...image, key: `${message.id}:${index}:${image.path}`, messageId: message.id }))), [visibleMessages]);
  const onChatMediaClick = (event: React.MouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    const link = target.closest('a.md-media-open') as HTMLAnchorElement | null;
    if (!link) return;
    event.preventDefault();
    const path = link.dataset.chatImagePath || '';
    const src = link.dataset.chatImageSrc || link.getAttribute('href') || '';
    const found = chatLightboxImages.find((item) => item.path === path || item.src === src);
    setChatImageModal(found || { key: `adhoc:${path || src}`, messageId: '', path, name: link.dataset.chatImageName || basename(path || src), src, downloadUrl: `${src}${src.includes('?') ? '&' : '?'}download=1` });
  };
  const contextWindowTotal = contextModelOption?.contextLength || fallbackContextWindowForModel(currentModel, sessionProvider);
  const contextWindowUsage = contextWindowTokens(props.messages, props.input, props.attachments, props.hasOlder || props.hasNewer, props.contextWindowSnapshot?.sessionId === props.activeSessionId ? props.contextWindowSnapshot : undefined);
  const preserveChatScrollForVisibilityChange = (nextShowReasoning: boolean, nextShowToolCalls: boolean, apply: () => void) => {
    const scroller = props.chatScrollRef.current;
    const nextVisibleMessages = visibleChatMessages<ChatMessage>(props.messages, nextShowReasoning, nextShowToolCalls);
    const nextVisibleIds = new Set(nextVisibleMessages.map((message) => String(message.id || '')).filter(Boolean));
    const anchor = captureMessageScrollAnchor(scroller, nextVisibleIds);
    apply();
    const restore = () => restoreMessageScrollAnchor(scroller, anchor);
    requestAnimationFrame(restore);
    window.setTimeout(restore, 60);
  };
  const toggleReasoningVisibility = () => preserveChatScrollForVisibilityChange(!props.showReasoning, props.showToolCalls, () => props.setShowReasoning(!props.showReasoning));
  const toggleToolCallVisibility = () => preserveChatScrollForVisibilityChange(props.showReasoning, !props.showToolCalls, () => props.setShowToolCalls(!props.showToolCalls));
  const lastAutoOlderRequestRef = useRef('');
  useEffect(() => {
    const scroller = props.chatScrollRef.current;
    const firstRawId = props.messages[0]?.id || '';
    if (!scroller || !firstRawId || !props.activeSessionId) return;
    if (!shouldAutoLoadOlderForHiddenHistory(scroller, props.hasOlder, props.loadingMessages)) return;
    const requestKey = `${props.activeSessionId}:${firstRawId}:${props.showReasoning ? 'r1' : 'r0'}:${props.showToolCalls ? 't1' : 't0'}`;
    if (lastAutoOlderRequestRef.current === requestKey) return;
    lastAutoOlderRequestRef.current = requestKey;
    props.loadMessageWindow(props.activeSessionId, 'older');
  }, [visibleMessages.length, props.messages[0]?.id, props.activeSessionId, props.hasOlder, props.loadingMessages, props.showReasoning, props.showToolCalls]);
  useLayoutEffect(() => {
    updateActiveNavigatorIds();
    updateSubagentWindow();
    const raf = requestAnimationFrame(updateActiveNavigatorIds);
    const windowRaf = requestAnimationFrame(updateSubagentWindow);
    const timer = window.setTimeout(() => { updateActiveNavigatorIds(); updateSubagentWindow(); }, 80);
    return () => { cancelAnimationFrame(raf); cancelAnimationFrame(windowRaf); window.clearTimeout(timer); };
  }, [visibleMessages.length, props.activeSessionId, props.userMessageNav, props.showReasoning, props.showToolCalls, updateActiveNavigatorIds, updateSubagentWindow]);
  useLayoutEffect(() => {
    const scroller = props.chatScrollRef.current;
    if (!scroller || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(scheduleSubagentWindowUpdate);
    observer.observe(scroller);
    scroller.querySelectorAll<HTMLElement>('[data-message-id]').forEach((row) => observer.observe(row));
    return () => observer.disconnect();
  }, [visibleMessages.length, props.activeSessionId, props.chatScrollRef, props.messages[0]?.id, props.messages.at(-1)?.id, scheduleSubagentWindowUpdate]);
  return <main className={`main-panel chat-main-panel ${props.desktopCompactMessages ? 'desktop-compact-chat' : ''}${isMobile ? ' mobile-compact-chat' : ''}`}>
    <header className="chat-header"><MobileHeaderDrawerButton open={props.mobileSidebarOpen} onClick={props.toggleMobileSidebar} /><div className="chat-header-copy"><h1>{activeTitle}</h1><div className="chat-header-meta"><span className={`chat-total-count${props.historyTotal === null ? ' loading' : ''}`} aria-busy={props.historyTotal === null}>{props.messages.length || 0} loaded · <span>{props.historyTotal ?? '—'} total</span></span><div className="mobile-chat-context"><ContextWindowMeter used={contextWindowUsage.used} approximate={contextWindowUsage.approximate} total={contextWindowTotal} /></div></div></div><div className="chat-header-actions"><div className="session-header-times" aria-label={t('chat.sessionTimes')}>{headerTimes.started && <time>{headerTimes.started}</time>}{headerTimes.latest && <time>{headerTimes.latest}</time>}</div><div className="desktop-chat-context"><ContextWindowMeter used={contextWindowUsage.used} approximate={contextWindowUsage.approximate} total={contextWindowTotal} /></div>
        <HeaderToolstrip theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings} /></div></header>
    <ChatUserNavigator items={props.userMessageNav || []} loading={props.userNavLoading} sessionId={props.activeSessionId} activeIds={activeNavigatorIds} onJumpToMessage={props.onJumpToMessage} chatScrollRef={props.chatScrollRef} />
    <div className="subagent-progress-overlay"><SubagentProgressCard sessionId={props.activeSessionId} beforeTime={subagentBeforeTime} showReasoning={props.showReasoning} showToolCalls={props.showToolCalls} compact={props.desktopCompactMessages} /></div>
    <section className="chat-scroll" ref={props.chatScrollRef} onScroll={onScroll} onClick={onChatMediaClick} onPointerDown={collapseComposerForHistory} onTouchStart={collapseComposerForHistory} onWheel={onWheel}>
      {props.loadingMessages && <div className="history-loading" aria-live="polite">{t('chat.loadingHistory')}</div>}
      {visibleMessages.length === 0 && <div className="empty-state chat-empty-state"><Bot className="big-mark" /><h2>{t('chat.inputPlaceholder')}</h2><p>{t('chat.emptyDesc')}</p></div>}
      <ChatTranscript
        messages={props.messages}
        showReasoning={props.showReasoning} showToolCalls={props.showToolCalls}
        streaming={props.streaming}
        assistantName={sessionModel || undefined}
        compact={props.desktopCompactMessages}
        newMessageBoundaryId={props.newMessageBoundaryId}
        loadTurnDetails={loadTranscriptTurnDetails}
        forceOpenLatestDetailToken={forceOpenLatestDetailToken}
      />
    </section>
    {(latestButtonVisible || isSmallLandscape || isFullscreen) && <div className="chat-latest-overlay chat-floating-controls">
      {latestButtonVisible && <button type="button" className="chat-latest-button" aria-label={t('chat.jumpLatest')} title={t('chat.jumpLatest')} onClick={jumpToLatest} disabled={props.loadingMessages}><ChevronDown aria-hidden="true" /></button>}
      {(isSmallLandscape || isFullscreen) && <button type="button" className="small-landscape-fullscreen-button" aria-label={isFullscreen ? t('chat.exitFullscreen') : t('chat.enterFullscreen')} title={isFullscreen ? t('chat.exitFullscreen') : t('chat.enterFullscreen')} onClick={toggleFullscreen}>{isFullscreen ? <Minimize2 aria-hidden="true" /> : <Maximize2 aria-hidden="true" />}</button>}
    </div>}
    <ChatImageLightbox items={chatLightboxImages} current={chatImageModal} onSelect={setChatImageModal} onClose={() => setChatImageModal(null)} />
    <footer className={`composer-wrap ${props.composerCompact ? 'composer-compact' : ''}`} ref={props.composerRef}>
      {props.newMessageCount > 0 && <button className="new-messages-bubble" onClick={props.onClearNewMessages} aria-label={t('chat.newMessages')}>{props.newMessageCount === 1 ? t('chat.newMessageCount') : t('chat.newMessagesCount').replace('{n}', String(props.newMessageCount))}</button>}
      <FollowUpQueueView items={props.followUpQueue || []} onSteer={props.onSteerQueuedItem} onEdit={props.onEditQueuedItem} onReorder={props.onReorderQueuedItem} />
      <div className="attachments">{props.attachments.map((a: Attachment) => <span className={`att ${a.kind}`} key={a.id}>{a.kind === 'image' ? <ImageIcon /> : <FileText />} {a.name} <button onClick={() => props.setAttachments((old: Attachment[]) => old.filter((x) => x.id !== a.id))}><X /></button></span>)}</div>
      <div className="composer-box" onDragOver={(e) => e.preventDefault()} onDrop={(e) => { e.preventDefault(); props.onFiles(e.dataTransfer.files); }}>
        <textarea ref={textareaRef} rows={1} value={props.input} onFocus={() => props.setComposerCompact(false)} onChange={(e) => props.setInput(e.target.value)} placeholder={t('chat.inputPlaceholder')} onKeyDown={(e) => { if (e.key !== 'Enter' || e.shiftKey || (e.nativeEvent as KeyboardEvent).isComposing) return; const modified = e.metaKey || e.ctrlKey; const shouldSend = props.composerEnterMode === 'enter-newline' ? modified : !modified; if (!shouldSend) return; e.preventDefault(); props.sendMessage(); }} />
        <div className="composer-footer">
          <input ref={props.fileInput} type="file" multiple hidden onChange={(e) => props.onFiles(e.target.files)} />
          <button className="icon-btn attach-btn" onClick={() => props.fileInput.current?.click()} title={t('chat.attachFiles')}><Paperclip /></button>
          <DropdownControl icon={<Bot />} ariaLabel={t('chat.model')} value={currentModel} valueProvider={sessionProvider} options={modelOptions} onChange={props.setModel} wide hideLabel searchable />
          <DropdownControl icon={<Brain />} ariaLabel={t('chat.reasoning')} value={props.effort} options={effortOptions} onChange={(value) => props.setEffort(value as (typeof EFFORTS)[number])} hideLabel />
          <button type="button" className={`icon-btn composer-view-toggle reasoning-view-toggle ${props.showReasoning ? 'active' : ''}`} aria-pressed={props.showReasoning} aria-label={props.showReasoning ? t('chat.hideThinking') : t('chat.showThinking')} title={props.showReasoning ? t('chat.hideThinking') : t('chat.showThinking')} onClick={toggleReasoningVisibility}><Lightbulb /></button>
          <button type="button" className={`icon-btn composer-view-toggle tool-call-view-toggle ${props.showToolCalls ? 'active' : ''}`} aria-pressed={props.showToolCalls} aria-label={props.showToolCalls ? t('chat.hideToolCalls') : t('chat.showToolCalls')} title={props.showToolCalls ? t('chat.hideToolCalls') : t('chat.showToolCalls')} onClick={toggleToolCallVisibility}><Terminal /></button>
          <button type="button" className={`icon-btn composer-view-toggle desktop-compact-view-toggle ${props.desktopCompactMessages ? 'active' : ''}`} aria-pressed={props.desktopCompactMessages} aria-label={t('chat.compactMode')} title={t('chat.compactMode')} onClick={() => props.setDesktopCompactMessages(!props.desktopCompactMessages)}><List /></button>
          <button type="button" className={`send-btn composer-primary-btn ${primaryActionIsStop ? 'is-stop' : 'is-send'}`} onClick={primaryActionIsStop ? props.stopStreaming : props.sendMessage} aria-label={primaryActionIsStop ? t('chat.stopStreaming') : t('chat.send')} title={primaryActionIsStop ? t('chat.stopStreaming') : t('chat.send')}>{primaryActionIsStop ? <Square /> : <ArrowUp />}</button>
        </div>
      </div>
    </footer>
  </main>;
}

function FollowUpQueueView({ items, onSteer, onEdit, onReorder }: { items: FollowUpQueueItem[]; onSteer: (item: FollowUpQueueItem) => void; onEdit: (item: FollowUpQueueItem) => void; onReorder: (fromIndex: number, toIndex: number) => void }) {
  const dragIdx = React.useRef<number | null>(null);
  const [dragOverIdx, setDragOverIdx] = React.useState<number | null>(null);
  if (!items.length) return null;
  return <div className="followup-queue" aria-label={t('chat.queuedFollowUps')}>
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
      <button type="button" className="followup-action" onClick={() => onSteer(item)} title={t('chat.steerNow')}>{t('chat.steer')}</button>
      <button type="button" className="followup-action" onClick={() => onEdit(item)} title={t('chat.editQueuedFollowUp')}><Pencil /></button>
    </div>)}
  </div>;
}

type WorkspacePreviewSetter = React.Dispatch<React.SetStateAction<WorkspacePreview>>;

type WorkspaceTreeProps = {
  rootEntries: WorkspaceEntry[];
  workspaceTree: Record<string, WorkspaceEntry[]>;
  expandedWorkspacePaths: Set<string>;
  toggleWorkspaceFolder: (entry: WorkspaceEntry) => void | Promise<void>;
  openWorkspaceEntry: (entry: WorkspaceEntry, options?: { edit?: boolean; route?: boolean }) => void | Promise<void>;
  downloadEntry: (entry: WorkspaceEntry) => void;
  openWorkspaceMenu?: (entry: WorkspaceEntry, event: React.MouseEvent) => void;
};

type WorkspaceAsideProps = WorkspaceTreeProps & {
  preview: WorkspacePreview;
  setPreview: WorkspacePreviewSetter;
  collapsed: boolean;
  setCollapsed: (collapsed: boolean) => void;
  openFullPreview: (path: string) => void;
};

type WorkspaceMainProps = {
  preview: WorkspacePreview;
  setPreview: WorkspacePreviewSetter;
  theme: Theme;
  setTheme: (theme: Theme) => void;
  mobileSidebarOpen: boolean;
  toggleMobileSidebar: () => void;
  mode: Mode;
  onNavigateToSettings: () => void;
};

function WorkspaceAside(props: WorkspaceAsideProps) {
  if (props.collapsed) return <aside className="workspace workspace-collapsed"><div className="workspace-collapsed-actions"><button className="workspace-rail-btn" title={t('workspace.expand')} aria-label={t('workspace.expand')} onClick={() => props.setCollapsed(false)}><ChevronLeft /></button></div></aside>;
  return <aside className="workspace"><WorkspaceBrowser rootEntries={props.rootEntries} workspaceTree={props.workspaceTree} expandedWorkspacePaths={props.expandedWorkspacePaths} toggleWorkspaceFolder={props.toggleWorkspaceFolder} openWorkspaceEntry={props.openWorkspaceEntry} downloadEntry={props.downloadEntry} preview={props.preview} setPreview={props.setPreview} compact setCollapsed={props.setCollapsed} openWorkspaceMenu={props.openWorkspaceMenu} openFullPreview={props.openFullPreview} /></aside>;
}
function WorkspaceMain({ preview, setPreview, theme, setTheme, mobileSidebarOpen, toggleMobileSidebar, mode, onNavigateToSettings }: WorkspaceMainProps) {
  return <main className="main-panel workspace-main"><header className="chat-header"><MobileHeaderDrawerButton open={mobileSidebarOpen} onClick={toggleMobileSidebar} /><div><h1>{t('workspace.title')}</h1><span>{t('workspace.editor')}</span></div><HeaderToolstrip theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><WorkspaceEditorPreview preview={preview} setPreview={setPreview} /></main>;
}

type WorkspaceTreeRowsProps = Pick<WorkspaceTreeProps, 'workspaceTree' | 'expandedWorkspacePaths' | 'toggleWorkspaceFolder' | 'downloadEntry' | 'openWorkspaceMenu'> & {
  entries: WorkspaceEntry[];
  openFile: (entry: WorkspaceEntry) => void | Promise<void>;
  depth?: number;
};

function WorkspaceTreeRows({ entries, workspaceTree, expandedWorkspacePaths, toggleWorkspaceFolder, openFile, downloadEntry, openWorkspaceMenu, depth = 0 }: WorkspaceTreeRowsProps) {
  return entries.map((entry) => {
    const expanded = entry.kind === 'dir' && expandedWorkspacePaths.has(entry.path);
    const children = expanded ? (workspaceTree[entry.path] || []) : [];
    const activate = () => entry.kind === 'dir' ? toggleWorkspaceFolder(entry) : openFile(entry);
    return <React.Fragment key={entry.path}>
      <div className={`file-row workspace-tree-row ${entry.kind} ${expanded ? 'expanded' : ''}`} style={{ paddingLeft: 10 + depth * 16 }} role="button" tabIndex={0} onClick={activate} onContextMenu={(event) => openWorkspaceMenu?.(entry, event)} onKeyDown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); activate(); } }}>
        <span className="caret">{entry.kind === 'dir' ? (expanded ? <ChevronDown /> : <ChevronRight />) : null}</span>{entry.kind === 'dir' ? <Folder /> : <FileText />}<span className="file-name">{entry.name}</span><span className="file-size">{entry.kind === 'file' ? formatFileSize(entry.size) : ''}</span>{entry.kind === 'file' && <button title={t('workspace.downloadFile')} onClick={(event) => { event.stopPropagation(); downloadEntry(entry); }}><Download /></button>}
      </div>
      {expanded && children.length > 0 && <WorkspaceTreeRows entries={children} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openFile={openFile} downloadEntry={downloadEntry} openWorkspaceMenu={openWorkspaceMenu} depth={depth + 1} />}
    </React.Fragment>;
  });
}

function WorkspaceSidebar({ rootEntries, workspaceTree, expandedWorkspacePaths, toggleWorkspaceFolder, openWorkspaceEntry, downloadEntry, openWorkspaceMenu }: WorkspaceTreeProps) {
  return <><div className="workspace-sidebar-head"><div><h2>{t('workspace.title')}</h2><p>{t('workspace.fileTree')}</p></div></div><div className="workspace-tree file-list"><WorkspaceTreeRows entries={rootEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openFile={openWorkspaceEntry} downloadEntry={downloadEntry} openWorkspaceMenu={openWorkspaceMenu} /></div></>;
}
function MarqueeText({ children }: { children: React.ReactNode }) {
  const rootRef = useRef<HTMLSpanElement>(null);
  const itemRef = useRef<HTMLSpanElement>(null);
  const [scrolling, setScrolling] = useState(false);
  useLayoutEffect(() => {
    const node = rootRef.current;
    const item = itemRef.current;
    if (!node || !item) return;
    const update = () => {
      const gap = 32;
      node.style.setProperty('--skill-subtitle-cycle', `${item.getBoundingClientRect().width + gap}px`);
      setScrolling(item.scrollWidth > node.clientWidth + 1);
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(node);
    observer.observe(item);
    return () => observer.disconnect();
  }, [children]);
  return <span ref={rootRef} className={`skill-subtitle-marquee ${scrolling ? 'scrolling' : ''}`}><span className="skill-subtitle-track"><span ref={itemRef} className="skill-subtitle-item">{children}</span>{scrolling && <span className="skill-subtitle-item" aria-hidden="true">{children}</span>}</span></span>;
}

function SkillsSidebar({ skills, activeSkillName, selectSkill, toggleSkillEnabled, openSkillMenu, filter, setFilter, expandedCats, setExpandedCats, closeMobileSidebar }: { skills: Skill[]; activeSkillName: string; selectSkill: (skill: Skill) => void; toggleSkillEnabled: (skill: Skill, enabled: boolean) => void; openSkillMenu: (skill: Skill, event: React.MouseEvent) => void; filter: string; setFilter: (v: string) => void; expandedCats: Set<string>; setExpandedCats: (v: Set<string>) => void; closeMobileSidebar: () => void }) {
  const enabledCount = skills.filter((skill) => skill.enabled !== false).length;
  const grouped = skills.reduce<Record<string, Skill[]>>((acc, skill) => { const cat = skill.category || 'uncategorized'; if (cat === '.archive') return acc; (acc[cat] ||= []).push(skill); return acc; }, {});
  const cats = Object.keys(grouped).sort();
  const query = filter.trim().toLowerCase();
  const filteredCats = query ? cats.filter((cat) => grouped[cat].some((s) => s.name.toLowerCase().includes(query) || (s.description || '').toLowerCase().includes(query))) : cats;
  const filteredSkills = (cat: string) => query ? grouped[cat].filter((s) => s.name.toLowerCase().includes(query) || (s.description || '').toLowerCase().includes(query)) : grouped[cat];
  const toggleCat = (cat: string) => setExpandedCats(new Set(expandedCats.has(cat) ? [...expandedCats].filter((c) => c !== cat) : [...expandedCats, cat]));
  return <><div className="cron-sidebar-head"><div><h2>{t('skills.title')}</h2><p>{enabledCount} {t('skills.enabledCount')} | {skills.length} {t('skills.installed')}</p></div></div><div className="session-searchbar"><input className="filter" placeholder={t('skills.search')} value={filter} onChange={(e) => setFilter(e.target.value)} style={{ gridColumn: '1 / -1' }} /></div><div className="skills-list sessions">{filteredCats.map((cat) => <React.Fragment key={cat}><div className="section-label" role="button" tabIndex={0} onClick={() => toggleCat(cat)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); toggleCat(cat); } }}>{expandedCats.has(cat) ? <ChevronDown /> : <ChevronRight />} {cat}</div>{expandedCats.has(cat) && filteredSkills(cat).map((skill) => <button type="button" className={`skill-row session-item ${skill.name === activeSkillName ? 'active' : ''}`} key={skill.name} onClick={() => { selectSkill(skill); closeMobileSidebar(); }} onContextMenu={(ev) => openSkillMenu(skill, ev)}><span className="session-text"><span className="session-title">{skill.name}{skill.version ? <span className="skill-version">{skill.version}</span> : null}</span><span className="session-preview">{skill.description || t('skills.noDescription')}</span></span><span className="skill-enable-toggle" role="switch" aria-checked={skill.enabled !== false} tabIndex={0} onClick={(ev) => { ev.stopPropagation(); toggleSkillEnabled(skill, !(skill.enabled !== false)); }} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); ev.stopPropagation(); toggleSkillEnabled(skill, !(skill.enabled !== false)); } }} /></button>)}</React.Fragment>)}</div></>;
}
type SkillBackup = { id?: string | number; version?: string | number; reason?: string };

type SkillMainProps = WorkspaceMainProps & {
  skill: Skill | null;
  showToast: (message: string) => void;
};

function SkillMain({ skill, preview, setPreview, theme, setTheme, mobileSidebarOpen, toggleMobileSidebar, mode, onNavigateToSettings, showToast: st }: SkillMainProps) {
  const [backups, setBackups] = useState<SkillBackup[]>([]);
  const [rollbacking, setRollbacking] = useState(false);
  useEffect(() => {
    setBackups([]);
    if (!skill) return;
    fetch(`/skills/backups?name=${encodeURIComponent(skill.name)}`, { cache: 'no-store' }).then((r) => r.ok ? r.json() : []).then((data) => setBackups(Array.isArray(data) ? data : [])).catch(() => {});
  }, [skill]);
  const doRollback = async (id: string) => {
    if (rollbacking) return;
    setRollbacking(true);
    try {
      const res = await fetch(`/skills/rollback/${encodeURIComponent(id)}`, { method: 'POST' });
      if (!res.ok) throw new Error(await res.text());
      const body = await res.json();
      if (body.ok) st(t('skills.rollbacked'));
      else st(tf('skills.rollbackFailed', body.message || 'unknown'));
    } catch (err) { st(tf('skills.rollbackFailed', errorMessage(err))); }
    setRollbacking(false);
  };
  const historyOptions = backups.filter((backup) => backup.id && backup.version).slice(0, 10).map((backup) => ({
    id: String(backup.id),
    label: String(backup.version),
    description: String(backup.reason || t('skills.snapshot')),
  }));
  return <main className="main-panel skills-main"><header className="chat-header"><MobileHeaderDrawerButton open={mobileSidebarOpen} onClick={toggleMobileSidebar} /><div className="skill-header-copy"><h1>{skill?.name || t('skills.title')}</h1><MarqueeText>{skill?.description || t('skills.select')}{skill?.version ? <span className="skill-version">{skill.version}</span> : null}</MarqueeText></div><HeaderToolstrip theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><WorkspaceEditorPreview preview={preview} setPreview={setPreview} emptyIcon={Puzzle} emptyTitle={t('skills.select')} emptyDesc={t('skills.selectHint')} saveUrl={skill ? (path: string) => `/skills/file?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(path)}` : undefined} toolbarExtra={skill ? <DropdownControl icon={<History />} ariaLabel={t('skills.backups')} value="" options={historyOptions} onChange={doRollback} placement="down" iconOnly className="skill-history-dropdown" emptyLabel={t('skills.noVersions')} /> : null} /></main>;
}
function SkillWorkspaceAside({ skill, skillFileTree, expandedSkillPaths, toggleSkillFolder, openSkillFile, openSkillFileMenu }: { skill: Skill | null; skillFileTree: Record<string, WorkspaceEntry[]>; expandedSkillPaths: Set<string>; toggleSkillFolder: (entry: WorkspaceEntry) => void; openSkillFile: (skillName: string, path: string) => void; openSkillFileMenu: (skill: Skill, entry: WorkspaceEntry, event: React.MouseEvent) => void }) {
  const triggerSkillDownload = (skill: Skill) => {
    const a = document.createElement('a');
    a.href = `/skills/download/${encodeURIComponent(skill.name)}`;
    a.download = `${skill.name}.zip`;
    a.click();
  };
  const renderRows = (entries: WorkspaceEntry[], depth = 0): React.ReactNode => entries.filter((e) => e.name !== '.archive').map((entry) => {
    const expanded = entry.kind === 'dir' && expandedSkillPaths.has(entry.path);
    const children = expanded ? (skillFileTree[entry.path] || []) : [];
    return <React.Fragment key={entry.path}>
      <div className={`file-row workspace-tree-row ${entry.kind} ${expanded ? 'expanded' : ''}`} style={{ paddingLeft: 10 + depth * 16 }} role="button" tabIndex={0} onClick={() => entry.kind === 'dir' ? toggleSkillFolder(entry) : skill && openSkillFile(skill.name, entry.path)} onContextMenu={(ev) => skill && openSkillFileMenu(skill, entry, ev)} onKeyDown={(ev) => { if (ev.key === 'Enter' || ev.key === ' ') { ev.preventDefault(); entry.kind === 'dir' ? toggleSkillFolder(entry) : skill && openSkillFile(skill.name, entry.path); } }}>
        <span className="caret">{entry.kind === 'dir' ? (expanded ? <ChevronDown /> : <ChevronRight />) : null}</span>{entry.kind === 'dir' ? <Folder /> : <FileText />}<span className="file-name">{entry.name}</span><span className="file-size">{entry.kind === 'file' ? formatFileSize(entry.size) : ''}</span>
      </div>
      {expanded && renderRows(children, depth + 1)}
    </React.Fragment>;
  });
  return <aside className="skill-workspace workspace"><div className="workspace-sidebar-head"><div><h2>{t('skills.skillFiles')}</h2><p>{skill?.category || t('skills.select')}</p></div>{skill && <button className="icon-btn" aria-label={t('skills.download')} title={t('skills.download')} onClick={() => triggerSkillDownload(skill)}><Download /></button>}</div><div className="workspace-tree file-list">{renderRows(skillFileTree[''] || [])}</div></aside>;
}
type WorkspaceEditorPreviewProps = {
  preview: WorkspacePreview;
  setPreview: WorkspacePreviewSetter;
  emptyIcon?: React.ComponentType<{ className?: string }>;
  emptyTitle?: string;
  emptyDesc?: string;
  saveUrl?: (path: string) => string;
  toolbarExtra?: React.ReactNode;
};

function WorkspaceEditorPreview({ preview, setPreview, emptyIcon, emptyTitle, emptyDesc, saveUrl, toolbarExtra }: WorkspaceEditorPreviewProps) {
  const [editMode, setEditMode] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [saving, setSaving] = useState(false);
  const startEdit = () => { setEditContent(preview.content || ''); setEditMode(true); };
  const cancelEdit = () => { setEditMode(false); setEditContent(''); };
  const copyEditContent = async () => { await navigator.clipboard.writeText(editContent); };
  useEffect(() => { setEditMode(false); setEditContent(''); }, [preview.path]);
  useEffect(() => { if (preview.editRequest && preview.kind === 'text') startEdit(); }, [preview.editRequest]);
  const saveEdit = async () => {
    setSaving(true);
    try {
      const target = saveUrl ? saveUrl(preview.path) : `/workspace/file?path=${encodeURIComponent(preview.path)}`;
      const res = await fetch(target, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ content: editContent }) });
      if (!res.ok) { alert(tf('workspace.saveFailed', res.status)); return; }
      setPreview({ ...preview, content: editContent });
      setEditMode(false);
      setEditContent('');
    } finally { setSaving(false); }
  };
  if (preview.kind === 'none') { const Icon = emptyIcon || Folder; return <section className="workspace-editor-preview empty"><div className="empty-state"><Icon className="big-mark" /><h2>{emptyTitle || t('workspace.selectFile')}</h2><p>{emptyDesc || t('workspace.selectFileDesc')}</p></div></section>; }
  const textPreview = isMarkdownPath(preview.path)
    ? <div className="workspace-markdown-preview md-content" dangerouslySetInnerHTML={{ __html: markdownText(preview.content || '') }} />
    : <pre className="workspace-code-highlight" dangerouslySetInnerHTML={{ __html: highlightSourceText(preview.content || '', preview.path) }} />;
  const readOnlyPreview = preview.kind === 'hex'
    ? <div className="workspace-hex-preview">{preview.truncated && <div className="workspace-hex-note">{tf('workspace.hexPreviewTruncated', formatFileSize(preview.totalSize || 0))}</div>}<pre className="workspace-hex-viewer">{preview.content}</pre></div>
    : <div className="workspace-text-preview">{textPreview}</div>;
  return <section className="workspace-editor-preview"><div className="preview-head"><span>{basename(preview.path)}</span><div className="preview-head-actions">{!editMode && preview.kind === 'text' && <button className="icon-btn" aria-label={t('workspace.edit')} onClick={startEdit}><Pencil /></button>}{editMode && <><button className="icon-btn" disabled={saving} onClick={saveEdit}><Save /></button><button className="icon-btn" aria-label={t('workspace.copyContent')} title={t('workspace.copyContent')} onClick={copyEditContent}><Copy /></button><button className="icon-btn" aria-label={t('workspace.cancelEdit')} onClick={cancelEdit}><X /></button></>}{!editMode && <button className="icon-btn" aria-label={t('workspace.closePreview')} onClick={() => setPreview({ path: '', content: '', kind: 'none' })}><X /></button>}{toolbarExtra}</div></div>{preview.kind === 'image' ? <div className="workspace-image-preview"><img src={preview.url} /></div> : editMode ? <div className="workspace-editor-overlay"><pre className="workspace-code-highlight workspace-editor-highlight" aria-hidden="true" dangerouslySetInnerHTML={{ __html: highlightSourceText(editContent || '', preview.path) + '\n' }} /><textarea className="workspace-editor-textarea" value={editContent} onChange={(e) => setEditContent(e.target.value)} spellCheck={false} onScroll={(e) => { const pre = e.currentTarget.previousElementSibling as HTMLElement; if (pre) { pre.scrollTop = e.currentTarget.scrollTop; pre.scrollLeft = e.currentTarget.scrollLeft; } }} /></div> : readOnlyPreview}</section>;
}
type WorkspaceBrowserProps = WorkspaceTreeProps & {
  preview: WorkspacePreview;
  setPreview: WorkspacePreviewSetter;
  compact: boolean;
  setCollapsed: (collapsed: boolean) => void;
  openFullPreview: (path: string) => void;
};

function WorkspaceBrowser({ rootEntries, workspaceTree, expandedWorkspacePaths, toggleWorkspaceFolder, openWorkspaceEntry, downloadEntry, preview, setPreview, compact, setCollapsed, openWorkspaceMenu, openFullPreview }: WorkspaceBrowserProps) {
  const openFile = (entry: WorkspaceEntry) => openWorkspaceEntry(entry, compact ? { route: false } : undefined);
  return <>
    <header className="workspace-head"><span className="panel-title">{t('workspace.title')}</span><span>{compact ? t('workspace.main') : t('workspace.full')}</span><button aria-label={compact ? t('workspace.collapse') : t('workspace.closePreview')} onClick={() => compact ? setCollapsed(true) : setPreview({ path: '', content: '', kind: 'none' })}><X /></button></header>
    <div className="workspace-tree file-list"><WorkspaceTreeRows entries={rootEntries} workspaceTree={workspaceTree} expandedWorkspacePaths={expandedWorkspacePaths} toggleWorkspaceFolder={toggleWorkspaceFolder} openFile={openFile} downloadEntry={downloadEntry} openWorkspaceMenu={openWorkspaceMenu} /></div>
    {preview.kind !== 'none' && <div className="preview"><div className="preview-head"><span>{basename(preview.path)}</span><div className="preview-head-actions"><button className="icon-btn" aria-label={t('workspace.openFullPreview')} title={t('workspace.openFullPreview')} onClick={() => openFullPreview(preview.path)}><Maximize2 /></button><button className="icon-btn" aria-label={t('workspace.closePreview')} onClick={() => setPreview({ path: '', content: '', kind: 'none' })}><X /></button></div></div>{preview.kind === 'image' ? <img src={preview.url} /> : preview.kind === 'hex' ? <pre className="workspace-hex-viewer compact">{preview.content}</pre> : isMarkdownPath(preview.path) ? <div className="workspace-markdown-preview compact md-content" dangerouslySetInnerHTML={{ __html: markdownText(preview.content || '') }} /> : <pre className="workspace-code-highlight" dangerouslySetInnerHTML={{ __html: highlightSourceText(preview.content || '', preview.path) }} />}</div>}
  </>;
}
function AdminMain({ mode, setStatus, showToast, theme, setTheme, onNavigateToSettings }: { mode: Extract<Mode, 'memory'>; apiBase: string; headers: (json?: boolean) => Record<string, string>; setStatus: (v: string) => void; showToast: (v: string) => void; theme: Theme; setTheme: (v: Theme) => void; onNavigateToSettings: () => void }) {
  return <main className={`main-panel admin-main ${mode === 'memory' ? 'memory-main' : ''}`}><header className="chat-header header-no-drawer"><div><h1>{t('memory.title')}</h1><span>{t('memory.subtitle')}</span></div><HeaderToolstrip theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings} /></header><MemoryPanel setStatus={setStatus} showToast={showToast} /></main>;
}
function CronSidebar({ jobs, editingId, beginCronEdit, resetCronForm, writeHashRoute, closeMobileSidebar }: { jobs: Job[]; editingId: string; beginCronEdit: (job: Job) => void; resetCronForm: () => void; writeHashRoute: (route: HashRoute) => void; closeMobileSidebar: () => void }) {
  return <><div className="cron-sidebar-head"><div><h2>{t('cron.jobs')}</h2><p>{jobs.length} {t('cron.scheduled')}</p></div><button className="new-chat-btn" aria-label={t('cron.newJob')} title={t('cron.newJob')} onClick={() => { resetCronForm(); writeHashRoute({ mode: 'cron' }); closeMobileSidebar(); }}><Plus /></button></div><div className="cron-sidebar-list">{jobs.map((j) => <button type="button" data-route={buildHashRoute({ mode: 'cron', jobId: jobId(j) })} className={`cron-sidebar-row ${jobId(j) === editingId ? 'active' : ''} ${jobState(j) === 'paused' ? 'paused' : ''}`} key={jobId(j)} onClick={() => { beginCronEdit(j); closeMobileSidebar(); }}>
    <span className="session-icon"><CalendarClock /></span><span className="session-text"><span className="session-title">{j.name || jobId(j)}</span><span className="session-preview">{jobSchedule(j.schedule)} · {jobStateLabel(j)}{j.script ? ` · ${j.script}` : ''}</span></span>
  </button>)}</div></>;
}
function cronOutputDisplayText(output: CronOutput | null, loading: boolean) {
  if (loading) return t('cron.loadingOutput');
  if (!output?.content) return t('cron.noOutput');
  return `${output.content}${output.truncated ? `\n\n${t('cron.outputTruncated')}` : ''}`;
}
function CronMain(props: { name: string; setName: (v: string) => void; schedule: string; setSchedule: (v: string) => void; prompt: string; setPrompt: (v: string) => void; script: string; setScript: (v: string) => void; deliver: string; setDeliver: (v: string) => void; editingId: string; currentJob: Job | null; cronOutput: CronOutput | null; cronOutputLoading: boolean; refreshCronOutput: () => void; saveCronJob: () => void; runCronJob: () => void; toggleCronPaused: () => void; deleteCronJob: () => void; theme: Theme; setTheme: (v: Theme) => void; mobileSidebarOpen: boolean; toggleMobileSidebar: () => void; mode: Mode; onNavigateToSettings: () => void }) {
  const pinnedModel = cronPinnedModel(props.currentJob);
  const paused = !!props.currentJob && jobState(props.currentJob) === 'paused';
  const [cronImageModal, setCronImageModal] = useState<ChatLightboxImage | null>(null);
  const cronOutputText = cronOutputDisplayText(props.cronOutput, props.cronOutputLoading);
  const cronLightboxImages = useMemo(() => chatMediaImagesFromMarkdown(cronOutputText).map((image, index) => ({ ...image, key: `cron:${props.editingId}:${index}:${image.path}`, messageId: props.editingId })), [cronOutputText, props.editingId]);
  const onCronOutputMediaClick = (event: React.MouseEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    const link = target.closest('a.md-media-open') as HTMLAnchorElement | null;
    if (!link) return;
    event.preventDefault();
    const path = link.dataset.chatImagePath || '';
    const src = link.dataset.chatImageSrc || link.getAttribute('href') || '';
    const found = cronLightboxImages.find((item) => item.path === path || item.src === src);
    setCronImageModal(found || { key: `cron-adhoc:${path || src}`, messageId: props.editingId, path, name: link.dataset.chatImageName || basename(path || src), src, downloadUrl: `${src}${src.includes('?') ? '&' : '?'}download=1` });
  };
  return <main className="main-panel cron-main">
    <header className="chat-header"><MobileHeaderDrawerButton open={props.mobileSidebarOpen} onClick={props.toggleMobileSidebar} /><div><h1>{props.editingId ? t('cron.editCron') : t('cron.newCron')}</h1><span>{t('cron.jobs')}</span></div><HeaderToolstrip className="cron-header-actions" theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings}><button type="button" aria-label={t('cron.saveAria')} title={t('cron.save')} className="icon-btn cron-action-btn" onClick={props.saveCronJob}><Save /></button><button type="button" aria-label={t('cron.runAria')} title={t('cron.runShort')} className="icon-btn cron-action-btn" disabled={!props.editingId} onClick={props.runCronJob}><PlayMark /></button><button type="button" aria-label={t(paused ? 'cron.resumeAria' : 'cron.pauseAria')} title={t(paused ? 'cron.resume' : 'cron.pause')} className="icon-btn cron-action-btn cron-pause-toggle" disabled={!props.editingId || !props.currentJob} onClick={props.toggleCronPaused}>{paused ? <Play /> : <Pause />}</button><button type="button" aria-label={t('cron.deleteAria')} title={t('cron.delete')} className="icon-btn cron-action-btn danger" disabled={!props.editingId} onClick={props.deleteCronJob}><Trash2 /></button></HeaderToolstrip></header>
    <section className="cron-detail-wrap"><div className="cron-detail">
      <label className="cron-field"><span>{t('cron.name')}</span><input value={props.name} onChange={(e) => props.setName(e.target.value)} placeholder={t('cron.placeholderName')} /></label>
      <label className="cron-field"><span>{t('cron.schedule')}</span><input value={props.schedule} onChange={(e) => props.setSchedule(e.target.value)} placeholder={t('cron.placeholderSchedule')} /></label>
      <label className="cron-field cron-prompt"><span>{t('cron.prompt')}</span><textarea value={props.prompt} onChange={(e) => props.setPrompt(e.target.value)} placeholder={t('cron.placeholderPrompt')} /></label>
      <label className="cron-field cron-script"><span>{t('cron.script')}</span><textarea value={props.script} onChange={(e) => props.setScript(e.target.value)} placeholder={t('cron.placeholderScript')} /></label>
      <label className="cron-field cron-fullwidth cron-deliver-field"><span>{t('cron.deliver')}</span><input value={props.deliver} onChange={(e) => props.setDeliver(e.target.value)} placeholder={t('cron.placeholderDeliver')} list="cron-deliver-options" /><datalist id="cron-deliver-options"><option value="origin">{t('cron.deliverOrigin')}</option><option value="local">{t('cron.deliverLocal')}</option><option value="all">{t('cron.deliverAll')}</option><option value="telegram" /><option value="weixin" /><option value="qqbot" /></datalist></label>
      {props.editingId && <section className="cron-model-field cron-fullwidth"><span>{t('cron.pinnedModel')}</span><div className="cron-model-value">{pinnedModel ? (pinnedModel.nonAgent ? <span className="cron-tool-chip muted">{t('cron.nonAgentJob')}</span> : <>{pinnedModel.provider && <span className="cron-tool-chip">{providerDisplayName(pinnedModel.provider)}</span>}{pinnedModel.model && <span className="cron-tool-chip">{pinnedModel.model}</span>}</>) : <span className="cron-tool-chip muted">{t('cron.noPinnedModel')}</span>}</div></section>}
      {props.editingId && <section className="cron-tools-field cron-fullwidth"><span>{t('cron.enabledTools')}</span><div className="cron-tool-list">{cronEnabledToolsets(props.currentJob).map((toolset) => <span className="cron-tool-chip" key={toolset}>{toolset}</span>)}{!cronEnabledToolsets(props.currentJob).length && <span className="cron-tool-chip muted">{t('cron.allDefaultTools')}</span>}</div></section>}
      {props.editingId && <section className="cron-output-panel cron-fullwidth" onClick={onCronOutputMediaClick}><div className="cron-output-head"><div className="cron-output-title">{props.cronOutput?.timestamp && <time className="cron-output-timestamp" dateTime={props.cronOutput.timestamp}>{props.cronOutput.timestamp}</time>}<span>{t('cron.lastOutput')}</span></div><button type="button" className="mobile-icon-only" onClick={props.refreshCronOutput} disabled={props.cronOutputLoading}><RefreshCw /> <span className="btn-label">{t('cron.refreshOutput')}</span></button></div><div className="cron-output-content md-content" dangerouslySetInnerHTML={{ __html: markdownText(cronOutputText) }} /></section>}
      <ChatImageLightbox items={cronLightboxImages} current={cronImageModal} onSelect={setCronImageModal} onClose={() => setCronImageModal(null)} />
    </div></section>
  </main>;
}
function MemoryPanel({ setStatus, showToast }: { setStatus: (v: string) => void; showToast: (v: string) => void }) {
  const [doc, setDoc] = useState<MemoryDoc>({ memory: '', user: '' });
  const load = useCallback(async () => { try { const res = await fetch('/memory'); if (!res.ok) throw new Error(await res.text()); setDoc(await res.json()); } catch (err) { setStatus(tf('status.memoryUnavailable', errorMessage(err))); } }, [setStatus]);
  useEffect(() => { load(); }, [load]);
  const save = async () => { const res = await fetch('/memory', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(doc) }); if (res.ok) { setStatus(t('memory.saved')); showToast(t('memory.saved')); } else setStatus(await res.text()); };
  return <section className="admin-content memory-grid"><label><span>MEMORY.md</span><textarea value={doc.memory} onChange={(e) => setDoc({ ...doc, memory: e.target.value })}/></label><label><span>USER.md</span><textarea value={doc.user} onChange={(e) => setDoc({ ...doc, user: e.target.value })}/></label><button className="save-memory" onClick={save}>{t('memory.save')}</button></section>;
}
function GitHubIcon() {
  return <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82A7.6 7.6 0 0 1 8 3.86c.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" /></svg>;
}

function SettingsMain(props: { apiServerUrl: string; apiBase: string; setApiBase: (v: string) => void; apiKey: string; setApiKey: (v: string) => void; loadModels: () => void; loadSessions: () => void; theme: Theme; setTheme: (v: Theme) => void; lang: Lang; setLang: (v: Lang) => void; followUpBehaviour: FollowUpBehaviour; setFollowUpBehaviour: (v: FollowUpBehaviour) => void; composerEnterMode: ComposerEnterMode; setComposerEnterMode: (v: ComposerEnterMode) => void; codeWrap: boolean; setCodeWrap: (v: boolean) => void; showToast: (v: string) => void }) {
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
    } catch (error) { setUpdateStatus('error'); setUpdateError(errorMessage(error, 'Check failed')); }
  };
  const saveSettings = () => { localStorage.setItem('apiBase', props.apiBase); localStorage.setItem('apiKey', props.apiKey); localStorage.setItem('theme', props.theme); localStorage.setItem(FOLLOW_UP_BEHAVIOUR_KEY, props.followUpBehaviour); localStorage.setItem(COMPOSER_ENTER_MODE_KEY, props.composerEnterMode); setI18nLang(props.lang); props.showToast(t('settings.saved')); };
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
    } catch (error) { setUpdateStatus('error'); setUpdateError(errorMessage(error, 'Update failed')); }
  };
  return <main className="main-panel settings-main"><header className="chat-header header-no-drawer"><div><h1>{t('settings.title')}</h1><span>{t('settings.summary')}</span></div><HeaderToolstrip theme={props.theme} setTheme={props.setTheme} mode={'settings' as Mode} /></header><section className="settings-content"><label><span>{t('settings.apiUrl')}</span><input value={props.apiServerUrl || '—'} readOnly /></label><label><span>{t('settings.apiProxyBase')}</span><input value={props.apiBase} onChange={(e) => props.setApiBase(e.target.value)} /></label><label><span>{t('settings.apiKey')}</span><input value={props.apiKey} onChange={(e) => props.setApiKey(e.target.value)} type="password" /></label><label><span>{t('settings.language')}</span><select value={props.lang} onChange={(e) => { const next = e.target.value as Lang; props.setLang(next); setI18nLang(next); }}>{LANG_OPTIONS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label><label><span>{t('settings.theme')}</span><select value={props.theme} onChange={(e) => props.setTheme(e.target.value as Theme)}>{THEME_OPTIONS.map((item) => <option key={item.id} value={item.id}>{item.label}</option>)}</select></label><label><span>{t('settings.followUpBehaviour')}</span><select value={props.followUpBehaviour} onChange={(e) => props.setFollowUpBehaviour(e.target.value as FollowUpBehaviour)}><option value="queue">{t('chat.queue')}</option><option value="steer">{t('chat.steer')}</option></select></label><label><span>{t('settings.composerEnterMode')}</span><select value={props.composerEnterMode} onChange={(e) => props.setComposerEnterMode(e.target.value as ComposerEnterMode)}><option value="enter-send">{t('settings.enterSend')}</option><option value="enter-newline">{t('settings.enterNewline')}</option></select></label><label><span>{t('settings.codeWrap')}</span><select value={props.codeWrap ? 'on' : 'off'} onChange={(e) => props.setCodeWrap(e.target.value === 'on')}><option value="on">{t('settings.codeWrapOn')}</option><option value="off">{t('settings.codeWrapOff')}</option></select></label><button className="mobile-icon-only settings-save-btn" aria-label={t('settings.save')} onClick={saveSettings}><Save /> <span className="btn-label">{t('settings.save')}</span></button><button className="mobile-icon-only" aria-label={t('settings.refreshConn')} onClick={() => { props.loadModels(); props.loadSessions(); }}><RefreshCw /> <span className="btn-label">{t('settings.refreshConn')}</span></button><div style={{ marginTop: 24, borderTop: '1px solid var(--border)', paddingTop: 16 }}><h3 style={{ margin: '0 0 8px', fontSize: 16 }}>{t('settings.update')}</h3><p style={{ margin: '0 0 10px', color: 'var(--muted)', fontSize: 13 }}>{t('settings.version')}: <code>{currentVer || '...'}</code></p><div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}><button className="btn-wide" onClick={checkForUpdates} disabled={updateStatus === 'checking' || updateStatus === 'applying' || updateStatus === 'restarting'} style={{ display: 'inline-flex', alignItems: 'center', gap: 6, padding: '8px 14px', border: '1px solid var(--border)', borderRadius: 12, background: 'var(--surface-2)', cursor: 'pointer', fontSize: 13 }}>{updateStatus === 'checking' ? t('settings.checkingUpdate') : <><RefreshCw size={15} /> {t('settings.checkUpdate')}</>}</button>{updateInfo && <span style={{ fontSize: 13 }}>{updateInfo.available ? <span style={{ color: 'var(--green)' }}>{t('settings.updateAvailable')}: {updateInfo.latest}</span> : <span style={{ color: 'var(--muted)' }}>{t('settings.upToDate')}</span>}</span>}{updateInfo?.available && updateInfo.release_url && <a href={updateInfo.release_url} target="_blank" rel="noopener noreferrer" style={{ fontSize: 13, color: 'var(--accent)' }}>{t('settings.viewRelease')}</a>}</div><div className="update-project-link-row"><a className="project-link" href="https://github.com/fffonion/yahu" target="_blank" rel="noopener noreferrer" aria-label="GitHub · fffonion/yahu"><GitHubIcon /> <span>GitHub · fffonion/yahu</span></a></div>{updateInfo?.available && <button className="btn-wide" onClick={applyUpdate} disabled={updateStatus === 'applying' || updateStatus === 'restarting'} style={{ marginTop: 10, display: 'inline-flex', alignItems: 'center', gap: 6, padding: '8px 14px', border: '1px solid var(--accent)', borderRadius: 12, background: 'var(--accent)', color: '#fff', cursor: 'pointer', fontSize: 13 }}>{updateStatus === 'applying' ? t('settings.installingUpdate') : updateStatus === 'restarting' ? t('settings.restartingUpdate') : <><Download size={15} /> {t('settings.installUpdate')}</>}</button>}{updateStatus === 'error' && <p style={{ margin: '8px 0 0', color: 'var(--danger)', fontSize: 13 }}>{updateError}</p>}</div></section></main>;
}

function ImageBrowser({ theme, setTheme, requestConfirm, initialImageFilename, writeHashRoute, mode, onNavigateToSettings }: { theme: Theme; setTheme: (v: Theme) => void; requestConfirm: (title: string, message: string, danger?: boolean) => Promise<boolean>; initialImageFilename?: string; writeHashRoute: (route: HashRoute) => void; mode?: Mode; onNavigateToSettings?: () => void }) {
  const MAX_PAGE_SIZE = 120;
  const MIN_PRELOAD_DISTANCE_PX = 1800;
  const GALLERY_PRELOAD_ROWS = 2;
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
  const modalPreloadLinkRef = useRef<HTMLLinkElement | null>(null);
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
  const imageUserScrolledRef = useRef(false);
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
  const downloadButtonLabel = (item: ImageEntry) => item.heic_status === 'missing' ? t('gallery.generateHeic') : t('gallery.download');
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
    const visibleRows = Math.max(1, Math.ceil(viewportH / (estimatedCardH + gap)));
    const rows = visibleRows + GALLERY_PRELOAD_ROWS;
    return clamp(cols * rows, cols * 2, MAX_PAGE_SIZE);
  };
  const lazyPageSizeForViewport = () => window.innerWidth <= 760 ? initialPageSizeForViewport() : clamp(getGridColumnCount() * 2, 4, MAX_PAGE_SIZE);
  const pageSizeForViewport = (offset: number) => offset === 0 ? initialPageSizeForViewport() : lazyPageSizeForViewport();
  const preloadDistancePx = () => Math.max(MIN_PRELOAD_DISTANCE_PX, Math.round(window.innerHeight * 2.5));
  const isNearImagePreloadWindow = (el: HTMLElement) => el.scrollHeight - el.scrollTop - el.clientHeight < preloadDistancePx();
  const shouldAutoLoadImages = () => imageUserScrolledRef.current || imagesRef.current.length < initialPageSizeForViewport();
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
  const openImageModal = (item: ImageEntry) => { setModal(item); setModalMetadataOpen(false); writeHashRoute({ mode: 'images', imageFilename: item.filename }); };
  const closeImageModal = () => { setModal(null); setModalMetadataOpen(false); writeHashRoute({ mode: 'images' }); };
  const removeImages = (names: string[], modalReplacement?: ImageEntry | null) => {
    const gone = new Set(names.filter(Boolean));
    const currentModal = modal;
    const nextModal = modalReplacement !== undefined ? modalReplacement : currentModal ? nextImageAfterRemoval(imagesRef.current, Array.from(gone), currentModal.filename) : null;
    const nextImages = imagesRef.current.filter((x) => !gone.has(x.filename));
    updateImages(nextImages);
    setSelected((old) => new Set(Array.from(old).filter((x) => !gone.has(x))));
    if (currentModal && gone.has(currentModal.filename)) {
      setModal(nextModal);
      setModalMetadataOpen(false);
      writeHashRoute(nextModal ? { mode: 'images', imageFilename: nextModal.filename } : { mode: 'images' });
    }
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
      .catch(() => setNotice(tf('gallery.imageNotFound', initialImageFilename)));
  }, [initialImageFilename]);
  const loadStats = useCallback(async () => {
    try { const res = await fetch('/image-api/stats', { cache: 'no-store' }); if (res.ok) setStats(await res.json()); }
    catch { /* ignore */ }
  }, []);
  const loadImages = useCallback(async (reset = false) => {
    if (loadingRef.current) { if (reset) reloadQueuedRef.current = true; return; }
    if (!reset && !hasMoreRef.current) return;
    if (reset) imageUserScrolledRef.current = false;
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
    } catch (err) { setNotice(`${t('gallery.imagesUnavailable')}: ${errorMessage(err)}`); }
    finally {
      loadingRef.current = false;
      setLoading(false);
      if (reloadQueuedRef.current) {
        reloadQueuedRef.current = false;
        loadImages(true);
      } else {
        window.requestAnimationFrame(maybeLoadImagesNearViewport);
      }
    }
  }, [loadStats]);
  const maybeLoadImagesNearViewport = () => {
    const el = scrollRef.current;
    if (!el || loadingRef.current || !hasMoreRef.current) return;
    if (isNearImagePreloadWindow(el) && shouldAutoLoadImages()) loadImages(false);
  };
  const refreshIncremental = useCallback(async () => {
    if (refreshBusyRef.current) return;
    refreshBusyRef.current = true;
    setNotice(t('gallery.refreshing'));
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
      if (added || updated) setNotice(tf('gallery.refreshComplete', added, updated));
      else if (window.innerWidth > 760) setNotice(t('gallery.refreshedNone'));
      await loadStats();
    } catch (err) { setNotice(`${t('gallery.refreshFailed')}: ${errorMessage(err)}`); }
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
      if (entries.some((entry) => entry.isIntersecting) && shouldAutoLoadImages()) loadImages(false);
    }, { root: scrollRef.current, rootMargin: `${preloadDistancePx()}px` });
    observer.observe(node);
    return () => observer.disconnect();
  }, [loadImages]);
  const onImageScroll = (event: React.UIEvent<HTMLElement>) => {
    const el = event.currentTarget;
    if (el.scrollTop > 0) imageUserScrolledRef.current = true;
    if (isNearImagePreloadWindow(el)) loadImages(false);
  };
  useEffect(() => {
    const es = new EventSource('/image-api/events');
    es.onopen = () => setNotice('');
    es.onmessage = (ev) => { try { const msg = JSON.parse(ev.data); if (msg.type === 'image' && msg.data) mergeImage(msg.data); if (msg.type === 'resync') refresh(); } catch { /* ignore */ } };
    es.addEventListener('delete', (ev) => { try { const msg = JSON.parse((ev as MessageEvent).data); removeImages(msg.filenames || [msg.filename]); } catch { /* ignore */ } });
    es.addEventListener('resync', refresh);
    es.onerror = () => setNotice(t('status.disconnected'));
    return () => es.close();
  }, [refresh]);
  useEffect(() => {
    if (!modal) { setMetadata(null); setModalMetadataOpen(false); return; }
    setMetadata(null);
    fetch(`/image-api/images/${enc(modal.filename)}/metadata`, { cache: 'no-store' }).then((res) => res.ok ? res.json() : null).then(setMetadata).catch(() => setMetadata(null));
  }, [modal?.filename]);
  const modalImageUrl = (item: ImageEntry) => item.png_url || item.image_url;
  const removeModalPreloadLink = () => {
    modalPreloadLinkRef.current?.remove();
    modalPreloadLinkRef.current = null;
  };
  useEffect(() => { resetModalImageMotion(); }, [modal?.filename]);
  useEffect(() => {
    removeModalPreloadLink();
    if (!modal) return;
    const next = nextImageForPreload(images, modal.filename);
    if (!next) return;
    const href = modalImageUrl(next);
    if (!href) return;
    const link = document.createElement('link');
    link.rel = 'preload';
    link.as = 'image';
    link.href = href;
    link.setAttribute('data-yahu-modal-preload', 'next');
    document.head.appendChild(link);
    modalPreloadLinkRef.current = link;
    return removeModalPreloadLink;
  }, [modal?.filename, images]);
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
      if (event.defaultPrevented) return;
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
    const nextModalAfterDelete = modal && names.includes(modal.filename) ? nextImageAfterRemoval(imagesRef.current, names, modal.filename) : null;
    if (!await requestConfirm(t('gallery.deleteImagesTitle'), tf('gallery.deleteConfirm', names.length), true)) return;
    const res = await fetch('/image-api/batch-delete', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ filenames: names }) });
    if (!res.ok) { setNotice(await res.text()); return; }
    removeImages(names, nextModalAfterDelete); setSelecting(false); await loadStats();
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
    setNotice(tf('gallery.generatingHeic', item.filename));
    const res = await fetch(`/image-api/images/${enc(item.filename)}/heic`, { method: 'POST' });
    if (!res.ok) { setNotice(await res.text()); return null; }
    const updated: ImageEntry = await res.json(); mergeImage(updated); setModal((old) => old?.filename === updated.filename ? updated : old); setNotice(t('gallery.heicDone'));
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
      <div><h1>{t('gallery.title')}</h1><span>{images.length}/{stats.total_images || '—'} {t('gallery.loaded')} · {formatImageBytes(stats.total_bytes)}{notice ? ` · ${notice}` : ''}</span></div>
      <HeaderToolstrip className="image-actions" theme={theme} setTheme={setTheme} mode={mode} onNavigateToSettings={onNavigateToSettings}>
        {selecting && selected.size > 0 && <><button className="icon-btn mobile-icon-only" aria-label={t('gallery.downloadSelected')} title={t('gallery.downloadSelected')} onClick={() => downloadSelectedFiles(selectedList)}><Download aria-hidden="true" /></button><button className="icon-btn mobile-icon-only" aria-label={t('gallery.organize')} title={t('gallery.organize')} onClick={organizeTime}><CalendarClock aria-hidden="true" /></button><button className="icon-btn danger mobile-icon-only" aria-label={t('gallery.deleteSelected')} title={t('gallery.deleteSelected')} onClick={() => deleteNames(selectedList)}><Trash2 aria-hidden="true" /></button></>}
        <button className={`icon-btn mobile-icon-only ${selecting ? 'active' : ''}`} aria-label={selecting ? t('gallery.cancelSelection') : t('gallery.selectImages')} title={selecting ? t('gallery.cancelSelection') : t('gallery.selectImages')} onClick={toggleSelectionMode}>{selecting ? <X aria-hidden="true" /> : <CheckSquare aria-hidden="true" />}</button>
        <button className="icon-btn" aria-label={t('gallery.refresh')} title={t('gallery.refresh')} onClick={refresh}><RefreshCw aria-hidden="true" /></button>
      </HeaderToolstrip>
    </header>
    <section className="image-grid-wrap" ref={scrollRef} onScroll={onImageScroll}>
      <div className="image-grid" ref={gridRef}>{images.map((item) => <article className={`image-card ${selecting ? 'selecting' : ''} ${selected.has(item.filename) ? 'selected' : ''}`} key={item.filename} onClick={() => selecting ? toggleSelect(item.filename) : openImageModal(item)} onContextMenu={(event) => { event.preventDefault(); setImageMenu({ item, x: event.clientX, y: event.clientY }); }}>
        {selecting && <button type="button" aria-label={tf('gallery.selectImage', item.filename)} className={`image-checkbox ${selected.has(item.filename) ? 'checked' : ''}`} onClick={(event) => { event.stopPropagation(); toggleSelect(item.filename); }} />}
        <img loading="eager" decoding="async" src={item.image_url} alt={item.filename} onLoad={(event) => event.currentTarget.classList.add('loaded')} />
        <div className="image-overlay"><span className="image-name" title={item.filename}>{item.filename}</span><button className="mobile-icon-only" aria-label={downloadButtonLabel(item)} onClick={(event) => { event.stopPropagation(); downloadOne(item); }}>{item.heic_status === 'missing' ? <RefreshCw /> : <Download />}</button></div>
      </article>)}</div>
      {images.length === 0 && !loading && <div className="empty-state"><ImageIcon className="big-mark" /><h2>{t('gallery.noImages')}</h2><p>{t('gallery.noImagesDesc')}</p></div>}
      <div ref={sentinelRef} className="image-sentinel">{loading ? t('gallery.loading') : hasMore ? t('gallery.scrollMore') : t('gallery.end')}</div>
    </section>
    {imageMenu && <div className="image-context-menu" role="menu" style={{ left: imageMenu.x, top: imageMenu.y }} onContextMenu={(event) => event.preventDefault()} onClick={() => setImageMenu(null)}>
      {imageMenu.item.heic_status !== 'not_applicable' && <button type="button" role="menuitem" onClick={() => { downloadOne(imageMenu.item); }}><Download /> {t('gallery.downloadHEIC')}</button>}
      <button type="button" role="menuitem" onClick={() => { triggerBrowserDownload(imageMenu.item.png_url || imageMenu.item.image_url, imageMenu.item.filename); }}><Download /> {t('gallery.downloadPNG')}</button>
      <button type="button" role="menuitem" className="danger" onClick={() => { deleteNames([imageMenu.item.filename]); }}><Trash2 /> {t('gallery.delete')}</button>
    </div>}
    {modal && <div className={`image-modal ${metadataPlacement === 'bottom' ? 'metadata-bottom' : ''} ${modalMetadataOpen ? 'metadata-open' : ''}`} onClick={onModalBackdropClick} onWheel={onModalWheel} onPointerDown={onModalPointerDown} onPointerMove={onModalPointerMove} onPointerUp={finishModalPointer} onPointerCancel={cancelModalPointer}>
      <img ref={modalImgRef} className="image-modal-img" src={modalImageUrl(modal)} alt={modal.filename} onLoad={adjustMetadataPlacement} onClick={onModalImageClick} />
      <aside className={`modal-meta ${metadataPlacement === 'bottom' ? 'metadata-bottom' : ''}`} onClick={(event) => event.stopPropagation()}>
        <h2>{t('gallery.metadata')}</h2>
        {metadata?.dimensions && <p className="metadata-dim">{t('gallery.dimensions')}: {metadata.dimensions.width} × {metadata.dimensions.height}</p>}
        <section className="metadata-files-section"><span>{t('gallery.files')}</span><p>PNG {metadata?.png ? formatImageBytes(metadata.png.size) : formatImageBytes(modal.size)}</p><p>WebP {metadata?.webp ? formatImageBytes(metadata.webp.size) : '—'}</p><p>HEIC {metadata?.heic ? formatImageBytes(metadata.heic.size) : modal.heic_status}</p></section>
        <section className="metadata-png-section"><span>{t('gallery.pngMetadata')}</span>{metadataEntries.length ? metadataEntries.map((entry) => <p key={entry.keyword}><b>{entry.keyword}</b><br />{entry.value}</p>) : <p>{t('gallery.noPngText')}</p>}</section>
      </aside>
      <div className="modalbar" onClick={(event) => event.stopPropagation()}>
        <button className="mobile-icon-only" aria-label={downloadButtonLabel(modal)} onClick={() => downloadOne(modal)}>{modal.heic_status === 'missing' ? <RefreshCw /> : <Download />}</button>
        <button className={`mobile-icon-only modal-metadata-toggle ${modalMetadataOpen ? 'active' : ''}`} aria-label={t('gallery.metadata')} aria-expanded={modalMetadataOpen} onClick={() => setModalMetadataOpen((value) => !value)}><Info /></button>
        <button className="mobile-icon-only" aria-label={t('gallery.previous')} disabled={modalIndex <= 0} onClick={() => navigateModal(-1)}><ChevronLeft /></button>
        <button className="mobile-icon-only" aria-label={t('gallery.next')} disabled={modalIndex < 0 || (!hasMore && modalIndex >= images.length - 1)} onClick={() => navigateModal(1)}><ChevronRight /></button>
        <button className="mobile-icon-only" aria-label={t('gallery.close')} onClick={closeImageModal}><X /></button>
        <button className="danger mobile-icon-only" aria-label={t('gallery.delete')} onClick={() => deleteNames([modal.filename])}><Trash2 /></button>
      </div>
    </div>}
  </main>;
}
