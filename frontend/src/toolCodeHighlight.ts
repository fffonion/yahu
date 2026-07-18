import { escapeHighlightedHtml, highlightSourceText } from './syntaxHighlight';

export type HighlightedToolCodeLine = {
  kind: 'context' | 'add' | 'remove' | 'file' | 'hunk' | 'meta';
  prefix: string;
  html: string;
  lineNumber?: string;
  filePath: string;
};

function normalizedDiffPath(value: string) {
  const path = value.trim().split(/\s+/)[0] || '';
  if (path === '/dev/null') return '';
  return path.replace(/^[ab]\//, '');
}

export function highlightedReadFileLines(content: string, filePath: string): HighlightedToolCodeLine[] {
  return String(content || '').split('\n').map((line) => {
    const match = line.match(/^\s*(\d+)\|(.*)$/);
    const source = match ? match[2] : line;
    return {
      kind: 'context',
      prefix: '',
      lineNumber: match?.[1] || '',
      html: highlightSourceText(source, filePath),
      filePath,
    };
  });
}

export function highlightedDiffLines(diff: string, fallbackFilePath: string): HighlightedToolCodeLine[] {
  let filePath = fallbackFilePath;
  return String(diff || '').split('\n').map((line) => {
    const targetMatch = line.match(/^\+\+\+\s+(.+)$/);
    if (targetMatch) filePath = normalizedDiffPath(targetMatch[1]) || filePath;
    const sourceMatch = line.match(/^---\s+(.+)$/);
    if (sourceMatch && !filePath) filePath = normalizedDiffPath(sourceMatch[1]);

    if (/^(?:---|\+\+\+)\s/.test(line) || /^diff --git\s/.test(line)) {
      return { kind: 'file', prefix: '', html: escapeHighlightedHtml(line), filePath };
    }
    if (/^@@/.test(line)) {
      return { kind: 'hunk', prefix: '', html: escapeHighlightedHtml(line), filePath };
    }
    if (/^(?:index |new file mode |deleted file mode |similarity index |rename from |rename to |\\ No newline|\*\*\*)/.test(line)) {
      return { kind: 'meta', prefix: '', html: escapeHighlightedHtml(line), filePath };
    }
    if (line.startsWith('+')) {
      return { kind: 'add', prefix: '+', html: highlightSourceText(line.slice(1), filePath), filePath };
    }
    if (line.startsWith('-')) {
      return { kind: 'remove', prefix: '-', html: highlightSourceText(line.slice(1), filePath), filePath };
    }
    const source = line.startsWith(' ') ? line.slice(1) : line;
    return { kind: 'context', prefix: line.startsWith(' ') ? ' ' : '', html: highlightSourceText(source, filePath), filePath };
  });
}
