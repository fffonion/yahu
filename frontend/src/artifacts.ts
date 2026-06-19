export type ArtifactRole = 'user' | 'assistant' | 'system' | 'tool';

export type ArtifactInputSession = {
  id: string;
  title?: string;
  preview?: string;
  startedAt?: string | number;
  started_at?: string | number;
  last_active?: string | number;
};

export type ArtifactInputMessage = {
  id?: string;
  role: ArtifactRole;
  content: string;
  timestamp?: string | number;
  toolName?: string;
};

export type ArtifactTimelineItem = {
  id: string;
  role: ArtifactRole;
  title: string;
  excerpt: string;
  timestamp?: string | number;
};

export type SessionArtifactVersion = {
  version: number;
  createdAt: number;
  summary: {
    totalMessages: number;
    userMessages: number;
    assistantMessages: number;
    toolMessages: number;
    systemMessages: number;
  };
  timeline: ArtifactTimelineItem[];
  highlights: string[];
};

export type SessionArtifact = {
  id: string;
  title: string;
  sourceSessionId: string;
  createdAt: number;
  updatedAt: number;
  versions: SessionArtifactVersion[];
};

export const ARTIFACTS_KEY = 'yahu.sessionArtifacts';

function cleanText(value: string) {
  return String(value || '').replace(/\s+/g, ' ').trim();
}

function excerpt(value: string, max = 180) {
  const text = cleanText(value);
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

function artifactIdForSession(sessionId: string) {
  return `artifact-${String(sessionId || 'draft').replace(/[^a-zA-Z0-9_-]+/g, '-')}`;
}

function artifactTitle(session: ArtifactInputSession) {
  return cleanText(session.title || session.preview || session.id || 'Untitled session') || 'Untitled session';
}

function readableList(items: string[], limit = 4) {
  const values = Array.from(new Set(items.map((item) => cleanText(item)).filter(Boolean))).slice(0, limit);
  return values.length ? values.join(', ') : 'none';
}

function messagesByRole(messages: ArtifactInputMessage[], role: ArtifactRole) {
  return messages.filter((message) => message.role === role && cleanText(message.content));
}

function summarizedExcerpt(label: string, group: ArtifactInputMessage[], max = 220) {
  if (!group.length) return '';
  const first = excerpt(group[0].content, 90);
  const last = excerpt(group[group.length - 1].content, 110);
  if (group.length === 1) return `${label}: ${first}`;
  return `${label}: ${group.length} entries. First: ${first} Latest: ${last}`;
}

function dashboardTimeline(messages: ArtifactInputMessage[]): ArtifactTimelineItem[] {
  const users = messagesByRole(messages, 'user');
  const assistants = messagesByRole(messages, 'assistant');
  const tools = messagesByRole(messages, 'tool');
  const systems = messagesByRole(messages, 'system');
  const rows: ArtifactTimelineItem[] = [];
  if (users.length) rows.push({ id: 'request', role: 'user', title: 'Request', excerpt: summarizedExcerpt('User asked', users), timestamp: users[0].timestamp });
  if (assistants.length) rows.push({ id: 'work-summary', role: 'assistant', title: 'Work summary', excerpt: summarizedExcerpt('Agent produced', assistants), timestamp: assistants[assistants.length - 1].timestamp });
  if (tools.length) rows.push({ id: 'tool-evidence', role: 'tool', title: 'Tool evidence', excerpt: `Tools used: ${readableList(tools.map((message) => message.toolName || 'tool'))}. Outputs checked: ${tools.length}. Latest: ${excerpt(tools[tools.length - 1].content, 110)}`, timestamp: tools[tools.length - 1].timestamp });
  if (systems.length) rows.push({ id: 'system-notes', role: 'system', title: 'System notes', excerpt: summarizedExcerpt('System context', systems), timestamp: systems[systems.length - 1].timestamp });
  return rows;
}

function timelineText(timeline: ArtifactTimelineItem[]) {
  return timeline.map((item) => `- ${item.title}: ${item.excerpt}`).join('\n');
}

function summarize(messages: ArtifactInputMessage[]) {
  return {
    totalMessages: messages.length,
    userMessages: messages.filter((message) => message.role === 'user').length,
    assistantMessages: messages.filter((message) => message.role === 'assistant').length,
    toolMessages: messages.filter((message) => message.role === 'tool').length,
    systemMessages: messages.filter((message) => message.role === 'system').length,
  };
}

function highlights(messages: ArtifactInputMessage[]) {
  const users = messagesByRole(messages, 'user');
  const assistants = messagesByRole(messages, 'assistant');
  const tools = messagesByRole(messages, 'tool');
  return [
    users[0] ? `Request: ${excerpt(users[0].content, 130)}` : '',
    assistants.length ? `Latest response: ${excerpt(assistants[assistants.length - 1].content, 130)}` : '',
    tools.length ? `Evidence: ${tools.length} tool output${tools.length === 1 ? '' : 's'} from ${readableList(tools.map((message) => message.toolName || 'tool'))}` : '',
  ].filter(Boolean);
}

export function buildSessionArtifact(input: { session: ArtifactInputSession; messages: ArtifactInputMessage[]; existing?: SessionArtifact; now?: number }): SessionArtifact {
  const now = input.now ?? Date.now();
  const sourceSessionId = input.session.id || 'draft';
  const base: SessionArtifact = input.existing || {
    id: artifactIdForSession(sourceSessionId),
    title: artifactTitle(input.session),
    sourceSessionId,
    createdAt: now,
    updatedAt: now,
    versions: [],
  };
  const messages = input.messages || [];
  const nextVersion: SessionArtifactVersion = {
    version: base.versions.length + 1,
    createdAt: now,
    summary: summarize(messages),
    timeline: dashboardTimeline(messages),
    highlights: highlights(messages),
  };
  return {
    ...base,
    title: artifactTitle(input.session),
    updatedAt: now,
    versions: [...base.versions, nextVersion],
  };
}

export function artifactCopyPrompt(artifact: SessionArtifact, version = artifact.versions[artifact.versions.length - 1]) {
  const latest = version;
  return [
    `Continue from artifact “${artifact.title}”.`,
    `Source session: ${artifact.sourceSessionId}`,
    `Version: ${latest?.version || 1}`,
    latest?.highlights?.length ? `Highlights:\n${latest.highlights.map((item) => `- ${item}`).join('\n')}` : '',
    latest?.timeline?.length ? `Timeline:\n${timelineText(latest.timeline)}` : '',
    'Use the artifact context above and propose the next concrete update.',
  ].filter(Boolean).join('\n\n');
}

export async function copyTextToClipboard(text: string, env: { navigator?: any; document?: any } = {}) {
  const nav = env.navigator ?? globalThis.navigator;
  const doc = env.document ?? globalThis.document;
  try {
    if (nav?.clipboard?.writeText) {
      await nav.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to textarea copy path.
  }
  if (!doc?.createElement || !doc?.body?.appendChild) return false;
  const textarea = doc.createElement('textarea');
  textarea.value = text;
  textarea.setAttribute?.('readonly', '');
  Object.assign(textarea.style || {}, { position: 'fixed', left: '-9999px', top: '0', opacity: '0' });
  doc.body.appendChild(textarea);
  textarea.focus?.();
  textarea.select?.();
  try {
    return !!doc.execCommand?.('copy');
  } catch {
    return false;
  } finally {
    textarea.remove?.();
  }
}

export function readStoredArtifacts(storage: Pick<Storage, 'getItem'> = localStorage): SessionArtifact[] {
  try {
    const parsed = JSON.parse(storage.getItem(ARTIFACTS_KEY) || '[]');
    return Array.isArray(parsed) ? parsed.filter((item) => item && typeof item.id === 'string' && Array.isArray(item.versions)) : [];
  } catch {
    return [];
  }
}
