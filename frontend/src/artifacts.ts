import { summarizeToolMessage } from './toolMessage';

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
  toolInput?: unknown;
};

export type ArtifactTimelineItem = {
  id: string;
  role: ArtifactRole;
  title: string;
  excerpt: string;
  timestamp?: string | number;
};

export type ArtifactEvidenceCategory = 'verification' | 'change' | 'observation' | 'diagnostic' | 'context' | 'other';

export type ArtifactEvidenceItem = {
  id: string;
  toolName: string;
  category: ArtifactEvidenceCategory;
  importance: number;
  title: string;
  summary: string;
  findings: string[];
  rawExcerpt: string;
  status: string;
  timestamp?: string | number;
};

export type ArtifactDocumentSection = {
  id: 'objective' | 'outcome' | 'verification' | 'changes' | 'observations' | 'open';
  title: string;
  items: string[];
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
  evidence: ArtifactEvidenceItem[];
  sections: ArtifactDocumentSection[];
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

function valueText(value: unknown) {
  if (value === undefined || value === null) return '';
  if (typeof value === 'string') return value;
  try { return JSON.stringify(value); }
  catch { return String(value); }
}

function commandText(input: unknown): string {
  if (!input) return '';
  if (typeof input === 'string') {
    try {
      const parsed = JSON.parse(input);
      return commandText(parsed) || input;
    } catch { return input; }
  }
  if (typeof input === 'object' && !Array.isArray(input)) {
    const record = input as Record<string, unknown>;
    const command = record.command ?? record.cmd ?? record.url ?? record.path ?? record.ref;
    if (typeof command === 'string') return command;
  }
  return '';
}

function normalizedToolName(name: string) {
  return name.replace(/^functions\./, '').toLowerCase();
}

function evidenceCategory(toolName: string, text: string, status: string): ArtifactEvidenceCategory {
  const name = normalizedToolName(toolName);
  const lower = text.toLowerCase();
  if (status !== 'ok') return 'diagnostic';
  if (/(bun test|cargo test|pytest|npm test|pnpm test|make test|pass\b|0 fail|build|vite build|make install|systemctl|health|status":"ok)/i.test(text)) return 'verification';
  if (/^(patch|write_file|skill_manage)$/.test(name) || /(diff --git|files? changed|insertions?\(\+\)|deletions?\(-\)|commit|installed \/)/i.test(text)) return 'change';
  if (name.startsWith('browser_') || /\bheading\b|\bbutton\b|snapshot|screenshot|console/i.test(text)) return 'observation';
  if (/^(read_file|search_files|web_search|web_extract)$/.test(name)) return 'context';
  return 'other';
}

function evidenceImportance(toolName: string, text: string, category: ArtifactEvidenceCategory, status: string) {
  const name = normalizedToolName(toolName);
  let score = 30;
  if (category === 'verification') score += 45;
  if (category === 'change') score += 35;
  if (category === 'observation') score += 32;
  if (category === 'diagnostic') score += 42;
  if (category === 'context') score -= 18;
  if (name === 'terminal' || name === 'process') score += 12;
  if (name.startsWith('browser_')) score += 8;
  if (/^(patch|write_file|skill_manage)$/.test(name)) score += 14;
  if (/^(read_file|search_files)$/.test(name)) score -= 12;
  if (status !== 'ok') score += 12;
  if (/(pass\b|0 fail|exit_code":0|exit code.*0|health|status":"ok|active\b|built in|finished `release`)/i.test(text)) score += 16;
  if (/(error|failed|traceback|panic|non-zero|exit_code":[1-9]|exit code.*[1-9])/i.test(text)) score += 18;
  return score;
}

function pushUnique(items: string[], value: string, max = 6) {
  const text = cleanText(value);
  if (!text || items.includes(text) || items.length >= max) return;
  items.push(text);
}

function outputTextForAnalysis(message: ArtifactInputMessage) {
  const summary = summarizeToolMessage(message.content, message.toolName || '', message.toolInput);
  const rawText = valueText(summary.result || summary.subtitle || message.content);
  const input = commandText(summary.input);
  const fields = summary.fields.map((field) => `${field.key}: ${field.value}`).join('\n');
  const combined = [summary.title, input, summary.subtitle, rawText, fields].filter(Boolean).join('\n');
  return { summary, rawText, input, combined };
}

function evidenceFindings(toolName: string, text: string, status: string) {
  const findings: string[] = [];
  const pass = text.match(/(\d+)\s+pass\b/i);
  if (pass) pushUnique(findings, `${pass[1]} tests passed`);
  const fail = text.match(/(\d+)\s+fail\b/i);
  if (fail) pushUnique(findings, `${fail[1]} failures reported`);
  if (/\b0\s+fail\b/i.test(text)) pushUnique(findings, '0 test failures reported');
  if (/health|"status"\s*:\s*"ok"|\bstatus\s*:\s*ok\b/i.test(text)) pushUnique(findings, 'Health/status check returned ok');
  if (/systemctl|\bactive\b/i.test(text)) pushUnique(findings, 'Service state was verified');
  if (/vite build|built in|bun run build|cargo build|finished `release`|make install/i.test(text)) pushUnique(findings, 'Build/install output completed');
  if (/files? changed|insertions?\(\+\)|deletions?\(-\)|diff --git/i.test(text)) pushUnique(findings, 'Code changes were captured from diff output');
  if (/\bheading\b|\bbutton\b|snapshot|screenshot/i.test(text)) pushUnique(findings, 'Browser-rendered UI was inspected');
  if (status !== 'ok') pushUnique(findings, `${toolName} reported ${status}`);
  if (!findings.length) pushUnique(findings, excerpt(text, 140));
  return findings;
}

function toolEvidence(messages: ArtifactInputMessage[]): ArtifactEvidenceItem[] {
  return messages
    .filter((message) => message.role === 'tool' && cleanText(message.content))
    .map((message, index) => {
      const { summary, rawText, input, combined } = outputTextForAnalysis(message);
      const category = evidenceCategory(summary.toolName, combined, summary.status);
      const importance = evidenceImportance(summary.toolName, combined, category, summary.status);
      const command = input ? ` — ${input}` : '';
      const findings = evidenceFindings(summary.toolName, combined, summary.status);
      return {
        id: message.id || `tool-${index}`,
        toolName: summary.toolName.replace(/^functions\./, ''),
        category,
        importance,
        title: summary.title,
        summary: `${summary.title}${command}: ${excerpt(summary.subtitle || rawText, 170)}`,
        findings,
        rawExcerpt: excerpt(rawText || summary.subtitle || message.content, 420),
        status: summary.status,
        timestamp: message.timestamp,
      };
    })
    .filter((item) => item.importance >= 55 && item.category !== 'context')
    .sort((a, b) => b.importance - a.importance)
    .slice(0, 8);
}

function summarizedExcerpt(label: string, group: ArtifactInputMessage[], max = 220) {
  if (!group.length) return '';
  const first = excerpt(group[0].content, 90);
  const last = excerpt(group[group.length - 1].content, 110);
  if (group.length === 1) return `${label}: ${first}`;
  return `${label}: ${group.length} entries. First: ${first} Latest: ${last}`;
}

function dashboardTimeline(messages: ArtifactInputMessage[], evidence: ArtifactEvidenceItem[]): ArtifactTimelineItem[] {
  const users = messagesByRole(messages, 'user');
  const assistants = messagesByRole(messages, 'assistant');
  const systems = messagesByRole(messages, 'system');
  const rows: ArtifactTimelineItem[] = [];
  if (users.length) rows.push({ id: 'request', role: 'user', title: 'Request', excerpt: summarizedExcerpt('User asked', users), timestamp: users[0].timestamp });
  if (assistants.length) rows.push({ id: 'work-summary', role: 'assistant', title: 'Work summary', excerpt: summarizedExcerpt('Agent produced', assistants), timestamp: assistants[assistants.length - 1].timestamp });
  if (evidence.length) {
    rows.push({
      id: 'tool-evidence',
      role: 'tool',
      title: 'Tool evidence',
      excerpt: evidence.slice(0, 3).map((item) => `${item.title}: ${item.findings[0] || item.summary}`).join(' · '),
      timestamp: evidence[0].timestamp,
    });
  }
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

function documentSections(messages: ArtifactInputMessage[], evidence: ArtifactEvidenceItem[]): ArtifactDocumentSection[] {
  const users = messagesByRole(messages, 'user');
  const assistants = messagesByRole(messages, 'assistant');
  const sections: ArtifactDocumentSection[] = [];
  if (users.length) sections.push({ id: 'objective', title: 'Objective', items: [excerpt(users[0].content, 220)] });
  if (assistants.length) sections.push({ id: 'outcome', title: 'Outcome', items: [excerpt(assistants[assistants.length - 1].content, 260)] });
  const verification = evidence.filter((item) => item.category === 'verification' || item.category === 'diagnostic').flatMap((item) => item.findings.map((finding) => `${item.title}: ${finding}`));
  if (verification.length) sections.push({ id: 'verification', title: 'Verification', items: verification.slice(0, 8) });
  const changes = evidence.filter((item) => item.category === 'change').flatMap((item) => item.findings.map((finding) => `${item.title}: ${finding}`));
  if (changes.length) sections.push({ id: 'changes', title: 'Changes', items: changes.slice(0, 6) });
  const observations = evidence.filter((item) => item.category === 'observation').flatMap((item) => item.findings.map((finding) => `${item.title}: ${finding}`));
  if (observations.length) sections.push({ id: 'observations', title: 'Observations', items: observations.slice(0, 6) });
  return sections;
}

function highlights(messages: ArtifactInputMessage[], evidence: ArtifactEvidenceItem[]) {
  const users = messagesByRole(messages, 'user');
  const assistants = messagesByRole(messages, 'assistant');
  return [
    users[0] ? `Request: ${excerpt(users[0].content, 130)}` : '',
    assistants.length ? `Latest response: ${excerpt(assistants[assistants.length - 1].content, 130)}` : '',
    evidence.length ? `Tool-backed evidence: ${evidence.slice(0, 3).map((item) => `${item.title} (${item.category})`).join(', ')}` : '',
  ].filter(Boolean);
}

function evidenceText(evidence: ArtifactEvidenceItem[]) {
  return evidence.map((item) => `- ${item.title} [${item.category}, ${item.status}]: ${item.findings.join('; ')}\n  ${item.rawExcerpt}`).join('\n');
}

function sectionText(sections: ArtifactDocumentSection[]) {
  return sections.map((section) => `${section.title}:\n${section.items.map((item) => `- ${item}`).join('\n')}`).join('\n\n');
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
  const evidence = toolEvidence(messages);
  const sections = documentSections(messages, evidence);
  const nextVersion: SessionArtifactVersion = {
    version: base.versions.length + 1,
    createdAt: now,
    summary: summarize(messages),
    timeline: dashboardTimeline(messages, evidence),
    highlights: highlights(messages, evidence),
    evidence,
    sections,
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
    latest?.sections?.length ? `Artifact brief:\n${sectionText(latest.sections)}` : '',
    latest?.highlights?.length ? `Highlights:\n${latest.highlights.map((item) => `- ${item}`).join('\n')}` : '',
    latest?.timeline?.length ? `Timeline:\n${timelineText(latest.timeline)}` : '',
    latest?.evidence?.length ? `Tool evidence:\n${evidenceText(latest.evidence)}` : '',
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
