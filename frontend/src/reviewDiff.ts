import { escapeHighlightedHtml, highlightSourceText } from './syntaxHighlight';

export type ReviewLineKind = 'hunk' | 'meta' | 'gap' | 'add' | 'remove' | 'context';
export type ReviewLine = { kind: ReviewLineKind; gap?: ReviewContextGap; oldLine?: number; newLine?: number; text: string; html: string; path: string };

export type ReviewDiffCounts = { added: number; removed: number };

export function parseUnifiedDiff(diff: string, fallbackPath: string, highlight = true): { lines: ReviewLine[]; counts: ReviewDiffCounts } {
  const lines = diff.split(/\r?\n/);
  if (lines[lines.length - 1] === '') lines.pop();

  const result: ReviewLine[] = [];
  const counts: ReviewDiffCounts = { added: 0, removed: 0 };
  let currentPath = fallbackPath;
  let inHunk = false;
  let oldLine = 0;
  let newLine = 0;
  let oldRemaining = 0;
  let newRemaining = 0;

  const pushRaw = (kind: 'hunk' | 'meta', text: string) => {
    result.push({ kind, text, html: escapeHighlightedHtml(text), path: currentPath });
  };
  const pushSource = (kind: 'add' | 'remove' | 'context', text: string, oldNumber?: number, newNumber?: number) => {
    if (kind === 'add') counts.added += 1;
    if (kind === 'remove') counts.removed += 1;
    result.push({ kind, oldLine: oldNumber, newLine: newNumber, text, html: highlight ? highlightSourceText(text, currentPath) : '', path: currentPath });
  };
  const finishHunkLine = () => {
    if (oldRemaining <= 0 && newRemaining <= 0) inHunk = false;
  };

  for (const line of lines) {
    const fileHeader = line.match(/^diff --git (.+) (.+)$/);
    if (fileHeader) {
      const destination = fileHeader[2]?.trim().replace(/^"|"$/g, '') ?? '';
      if (destination && destination !== '/dev/null') currentPath = destination.replace(/^b\//, '');
      inHunk = false;
      continue;
    }

    const hunkHeader = line.match(/^@@ -(?<oldStart>\d+)(?:,(?<oldCount>\d+))? \+(?<newStart>\d+)(?:,(?<newCount>\d+))? @@(?:.*)$/);
    if (hunkHeader?.groups) {
      oldLine = Number(hunkHeader.groups.oldStart);
      newLine = Number(hunkHeader.groups.newStart);
      oldRemaining = Number(hunkHeader.groups.oldCount ?? 1);
      newRemaining = Number(hunkHeader.groups.newCount ?? 1);
      inHunk = oldRemaining > 0 || newRemaining > 0;
      pushRaw('hunk', line);
      continue;
    }

    if (inHunk && line === '\\ No newline at end of file') {
      pushRaw('meta', line);
      continue;
    }
    if (inHunk && line.startsWith('+') && newRemaining > 0) {
      pushSource('add', line.slice(1), undefined, newLine);
      newLine += 1;
      newRemaining -= 1;
      finishHunkLine();
      continue;
    }
    if (inHunk && line.startsWith('-') && oldRemaining > 0) {
      pushSource('remove', line.slice(1), oldLine > 0 ? oldLine : undefined, undefined);
      oldLine += 1;
      oldRemaining -= 1;
      finishHunkLine();
      continue;
    }
    if (inHunk && line.startsWith(' ') && oldRemaining > 0 && newRemaining > 0) {
      pushSource('context', line.slice(1), oldLine > 0 ? oldLine : undefined, newLine > 0 ? newLine : undefined);
      oldLine += 1;
      newLine += 1;
      oldRemaining -= 1;
      newRemaining -= 1;
      finishHunkLine();
      continue;
    }

    if (/^---\s/.test(line)) {
      const oldPath = line.slice(4).split('\t', 1)[0]?.trim() ?? '';
      if (!currentPath && oldPath !== '/dev/null') currentPath = oldPath.replace(/^a\//, '').replace(/^"|"$/g, '');
      continue;
    }
    if (/^\+\+\+\s/.test(line)) {
      const newPath = line.slice(4).split('\t', 1)[0]?.trim() ?? '';
      if (newPath && newPath !== '/dev/null') currentPath = newPath.replace(/^b\//, '').replace(/^"|"$/g, '');
      continue;
    }
    if (line.startsWith('index ')) continue;

    pushRaw('meta', line);
  }

  return { lines: result, counts };
}

export type ReviewContextGap = { before: number; up: boolean; down: boolean };
export type ReviewContextDirection = 'up' | 'down';
export type ReviewContextExpansion = { top: number; bottom: number };
export const REVIEW_CONTEXT_STEP = 20;

export function reviewContextGaps(lines: ReviewLine[]): ReviewContextGap[] {
  const gaps: ReviewContextGap[] = [];
  let oldEnd = 0;
  let newEnd = 0;
  let seenHunk = false;
  for (const [before, line] of lines.entries()) {
    if (line.kind !== 'hunk') continue;
    const hunk = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(line.text);
    if (!hunk) return [];
    const oldCount = Number(hunk[2] ?? 1);
    const newCount = Number(hunk[4] ?? 1);
    // Empty ranges name the line just before the insertion/deletion.
    const oldBefore = Number(hunk[1]) - (oldCount ? 1 : 0);
    const newBefore = Number(hunk[3]) - (newCount ? 1 : 0);
    if (!seenHunk && ((oldBefore === 0 && !oldCount) || (newBefore === 0 && !newCount))) return [];
    if (oldBefore > oldEnd && newBefore > newEnd) gaps.push({ before, up: true, down: seenHunk });
    oldEnd = oldBefore + oldCount;
    newEnd = newBefore + newCount;
    seenHunk = true;
  }
  // The compact patch does not carry the file's final line count.
  if (seenHunk) gaps.push({ before: lines.length, up: false, down: true });
  return gaps;
}

export function collectReviewContext(compact: ReviewLine[], full: ReviewLine[]): Map<number, ReviewLine[]> | null {
  const source = (lines: ReviewLine[]) => lines.filter(l => l.kind === 'add' || l.kind === 'remove' || l.kind === 'context');
  const expanded = source(full);
  if (full.filter(l => l.kind === 'hunk').length !== 1) return null;
  const context = new Map<number, ReviewLine[]>();
  const same = (a: ReviewLine, b: ReviewLine) => a.kind === b.kind && a.oldLine === b.oldLine
    && a.newLine === b.newLine && a.text === b.text && a.path === b.path;
  let offset = 0;
  let before: number | null = null;
  for (const [index, line] of compact.entries()) {
    if (line.kind === 'hunk') { before = index; continue; }
    if (line.kind === 'meta') continue;
    const start = offset;
    while (expanded[offset]?.kind === 'context' && !same(expanded[offset]!, line)) offset += 1;
    if (offset > start) {
      if (before === null) return null;
      context.set(before, expanded.slice(start, offset));
    }
    if (!expanded[offset] || !same(expanded[offset]!, line)) return null;
    offset += 1;
    before = null;
  }
  const tail = expanded.slice(offset);
  if (tail.some(line => line.kind !== 'context')) return null;
  if (tail.length) context.set(compact.length, tail);
  return context;
}

export function expandReviewContext(previous: ReviewContextExpansion, direction: ReviewContextDirection, total: number): ReviewContextExpansion {
  const step = Math.min(REVIEW_CONTEXT_STEP, Math.max(0, total - previous.top - previous.bottom));
  return direction === 'up' ? { ...previous, bottom: previous.bottom + step } : { ...previous, top: previous.top + step };
}

export function reviewLinesWithContext(
  lines: ReviewLine[],
  gaps: ReviewContextGap[],
  context: Map<number, ReviewLine[]> | null,
  revealed: Record<number, ReviewContextExpansion>,
): ReviewLine[] {
  if (!gaps.length) return lines;
  const result: ReviewLine[] = [];
  let next = 0;
  for (const gap of gaps) {
    for (; next < gap.before; next += 1) result.push(lines[next]!);
    const hidden = context?.get(gap.before) ?? [];
    const { top, bottom } = revealed[gap.before] ?? { top: 0, bottom: 0 };
    const highlight = (line: ReviewLine) => ({ ...line, html: highlightSourceText(line.text, line.path) });
    result.push(...hidden.slice(0, top).map(highlight));
    if (!context || top + bottom < hidden.length) {
      result.push({ kind: 'gap', gap, path: '', text: '', html: '' });
    }
    if (bottom) result.push(...hidden.slice(-bottom).map(highlight));
  }
  for (; next < lines.length; next += 1) result.push(lines[next]!);
  return result;
}

export function newLineAnchors(lines: ReviewLine[]): Map<ReviewLine, number> {
  const anchors = new Map<ReviewLine, number>();
  const nextNew = new Map<ReviewLine, number>();
  let next: number | undefined;
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]!;
    if (line.kind === 'hunk') next = undefined;
    else if (line.newLine !== undefined) next = line.newLine;
    else if (line.kind === 'remove' && next !== undefined) nextNew.set(line, next);
  }
  let previous: number | undefined;
  let hunkStart = 1;
  for (const line of lines) {
    if (line.kind === 'hunk') {
      previous = undefined;
      hunkStart = Math.max(1, Number(line.text.match(/\+(\d+)/)?.[1] ?? 1));
    } else if (line.newLine !== undefined) {
      previous = line.newLine;
      anchors.set(line, line.newLine);
    } else if (line.kind === 'remove') {
      anchors.set(line, nextNew.get(line) ?? previous ?? hunkStart);
    }
  }
  return anchors;
}

export type SplitRow = { index: number; left?: ReviewLine; right?: ReviewLine };

export function buildSplitRows(lines: ReviewLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  const append = (left?: ReviewLine, right?: ReviewLine) => {
    rows.push({ index: rows.length, left, right });
  };

  for (let index = 0; index < lines.length;) {
    const line = lines[index]!;
    if (line.kind === 'remove') {
      const removed: ReviewLine[] = [];
      while (index < lines.length && lines[index]?.kind === 'remove') {
        removed.push(lines[index]!);
        index += 1;
      }
      const added: ReviewLine[] = [];
      while (index < lines.length && lines[index]?.kind === 'add') {
        added.push(lines[index]!);
        index += 1;
      }
      const pairCount = Math.max(removed.length, added.length);
      for (let pair = 0; pair < pairCount; pair += 1) append(removed[pair], added[pair]);
      continue;
    }

    if (line.kind === 'add') {
      append(undefined, line);
    } else if (line.kind === 'context') {
      append(line, line);
    } else {
      append(line, undefined);
    }
    index += 1;
  }

  return rows;
}

export function numberedFileLines(content: string, filePath: string): ReviewLine[] {
  const body = content.replace(/^\uFEFF/, '');
  if (body.length === 0) return [];
  const lines = body.split(/\r?\n/);
  if (lines[lines.length - 1] === '') lines.pop();

  return lines.map((text, index) => ({
    kind: 'context',
    newLine: index + 1,
    text,
    html: highlightSourceText(text, filePath),
    path: filePath,
  }));
}
