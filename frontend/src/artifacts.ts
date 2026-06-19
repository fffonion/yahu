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

const ROLE_TITLE: Record<ArtifactRole, string> = {
  user: 'User request',
  assistant: 'Agent response',
  system: 'System note',
  tool: 'Tool output',
};

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

function timelineTitle(message: ArtifactInputMessage) {
  if (message.role === 'tool') return message.toolName || ROLE_TITLE.tool;
  return ROLE_TITLE[message.role] || message.role;
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
  const candidates = messages
    .filter((message) => message.role === 'user' || message.role === 'assistant')
    .map((message) => excerpt(message.content, 140))
    .filter(Boolean);
  return candidates.slice(-4).reverse();
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
    timeline: messages.slice(-12).map((message, index) => ({
      id: message.id || `message-${index}`,
      role: message.role,
      title: timelineTitle(message),
      excerpt: excerpt(message.content),
      timestamp: message.timestamp,
    })),
    highlights: highlights(messages),
  };
  return {
    ...base,
    title: artifactTitle(input.session),
    updatedAt: now,
    versions: [...base.versions, nextVersion],
  };
}

export function artifactCopyPrompt(artifact: SessionArtifact) {
  const latest = artifact.versions[artifact.versions.length - 1];
  return [
    `Continue from artifact “${artifact.title}”.`,
    `Source session: ${artifact.sourceSessionId}`,
    `Version: ${latest?.version || 1}`,
    latest?.highlights?.length ? `Recent highlights:\n${latest.highlights.map((item) => `- ${item}`).join('\n')}` : '',
    'Use the artifact context above and propose the next concrete update.',
  ].filter(Boolean).join('\n\n');
}

export function readStoredArtifacts(storage: Pick<Storage, 'getItem'> = localStorage): SessionArtifact[] {
  try {
    const parsed = JSON.parse(storage.getItem(ARTIFACTS_KEY) || '[]');
    return Array.isArray(parsed) ? parsed.filter((item) => item && typeof item.id === 'string' && Array.isArray(item.versions)) : [];
  } catch {
    return [];
  }
}
