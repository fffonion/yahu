import { escapeHighlightedHtml, highlightSourceText } from './syntaxHighlight';

export type ReviewLineKind = 'file' | 'hunk' | 'meta' | 'add' | 'remove' | 'context';
export type ReviewLine = { kind: ReviewLineKind; oldLine?: number; newLine?: number; text: string; html: string; path: string };

export function parseUnifiedDiff(diff: string, fallbackPath: string): ReviewLine[] {
  const lines = diff.split(/\r?\n/);
  if (lines[lines.length - 1] === '') lines.pop();

  const result: ReviewLine[] = [];
  let currentPath = fallbackPath;
  let inHunk = false;
  let oldLine = 0;
  let newLine = 0;
  let oldRemaining = 0;
  let newRemaining = 0;

  const pushRaw = (kind: 'file' | 'hunk' | 'meta', text: string) => {
    result.push({ kind, text, html: escapeHighlightedHtml(text), path: currentPath });
  };
  const pushSource = (kind: 'add' | 'remove' | 'context', text: string, oldNumber?: number, newNumber?: number) => {
    result.push({ kind, oldLine: oldNumber, newLine: newNumber, text, html: highlightSourceText(text, currentPath), path: currentPath });
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
      pushRaw('file', line);
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
      pushRaw('file', line);
      continue;
    }
    if (/^\+\+\+\s/.test(line)) {
      const newPath = line.slice(4).split('\t', 1)[0]?.trim() ?? '';
      if (newPath && newPath !== '/dev/null') currentPath = newPath.replace(/^b\//, '').replace(/^"|"$/g, '');
      pushRaw('file', line);
      continue;
    }
    if (/^(?:index |new file mode |deleted file mode |old mode |new mode |similarity index |dissimilarity index |rename from |rename to |copy from |copy to |Binary files |GIT binary patch|literal |delta )/.test(line) || line === '\\ No newline at end of file') {
      pushRaw('meta', line);
      continue;
    }

    pushRaw('meta', line);
  }

  return result;
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
