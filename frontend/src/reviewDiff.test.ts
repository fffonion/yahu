import { describe, expect, test } from 'bun:test';
import { buildSplitRows, collectReviewContext, expandReviewContext, newLineAnchors, numberedFileLines, parseUnifiedDiff, reviewContextGaps, reviewLinesWithContext, type ReviewLine } from './reviewDiff';

describe('review context', () => {
  const compact = parseUnifiedDiff([
    '@@ -3 +3,2 @@', '-old', '+new', '+added', '@@ -6 +7 @@', '-before', '+after',
  ].join('\n'), 'demo.ts').lines;
  const full = parseUnifiedDiff([
    '@@ -1,7 +1,8 @@', ' first', ' second', '-old', '+new', '+added',
    ' fourth', ' fifth', '-before', '+after', ' last',
  ].join('\n'), 'demo.ts').lines;

  test('finds and verifies the contexts before, between and after changed blocks', () => {
    const gaps = reviewContextGaps(compact);
    expect(gaps).toEqual([
      { before: 0, up: true, down: false },
      { before: 4, up: true, down: true },
      { before: 7, up: false, down: true },
    ]);
    const context = collectReviewContext(compact, full);
    expect([...context!].map(([before, rows]) => [before, rows.map(l => [l.oldLine, l.newLine, l.text])])).toEqual([
      [0, [[1, 1, 'first'], [2, 2, 'second']]],
      [4, [[4, 5, 'fourth'], [5, 6, 'fifth']]],
      [7, [[7, 8, 'last']]],
    ]);
    expect(collectReviewContext(compact, full.map(l => l.kind === 'add' ? { ...l, text: 'stale' } : l))).toBeNull();
    const display = reviewLinesWithContext(compact, gaps, context, { 4: { top: 0, bottom: 1 } });
    expect(display.map(l => l.kind === 'gap' ? 'gap' : l.text)).toEqual(['gap', '@@ -3 +3,2 @@', 'old', 'new', 'added', 'gap', 'fifth', '@@ -6 +7 @@', 'before', 'after', 'gap']);
    expect(buildSplitRows(display).find(row => row.left?.text === 'fifth')).toMatchObject({ left: { oldLine: 5 }, right: { newLine: 6 } });
  });

  test('does not offer context for a whole-file addition or deletion', () => {
    for (const diff of ['@@ -0,0 +1,2 @@\n+one\n+two', '@@ -1,2 +0,0 @@\n-one\n-two']) {
      expect(reviewContextGaps(parseUnifiedDiff(diff, 'demo.ts').lines)).toEqual([]);
    }
  });

  test('reveals 20 lines from the selected edge without repeating any lines', () => {
    const zero = { top: 0, bottom: 0 };
    const upper = expandReviewContext(zero, 'down', 42);
    const both = expandReviewContext(upper, 'up', 42);
    expect(upper).toEqual({ top: 20, bottom: 0 });
    expect(both).toEqual({ top: 20, bottom: 20 });
    expect(expandReviewContext(both, 'down', 42)).toEqual({ top: 22, bottom: 20 });
  });
});

describe('review diff helpers', () => {
  test('parses multiple hunks and files with accurate old and new line numbers', () => {
    const diff = [
      'diff --git a/src/one.ts b/src/one.ts',
      'index 1111111..2222222 100644',
      '--- a/src/one.ts',
      '+++ b/src/one.ts',
      '@@ -1,4 +1,5 @@ first hunk',
      ' const old = 1;',
      '-const gone = "<old>";',
      '+const added = "<new>";',
      ' const kept = true;',
      '\\ No newline at end of file',
      '@@ -10,2 +11,2 @@ second hunk',
      '-before',
      '+after',
      ' same',
      'diff --git a/src/new.ts b/src/new.ts',
      'new file mode 100644',
      '--- /dev/null',
      '+++ b/src/new.ts',
      '@@ -0,0 +1,3 @@ empty old side',
      '+export const one = 1;',
      '+export const two = 2;',
      '+export const three = 3;',
      '\\ No newline at end of file',
    ].join('\n');

    const { lines, counts } = parseUnifiedDiff(diff, 'fallback.ts');
    expect(counts).toEqual({ added: 5, removed: 2 });
    expect(lines.map((line) => line.text)).not.toContain('index 1111111..2222222 100644');
    expect(lines[0]?.text).toBe('@@ -1,4 +1,5 @@ first hunk');
    expect(lines.some(({ text }) => /^(?:diff --git |--- |\+\+\+ )/.test(text))).toBe(false);
    const removed = lines.filter((line) => line.kind === 'remove');
    const added = lines.filter((line) => line.kind === 'add');
    const context = lines.filter((line) => line.kind === 'context');
    const noNewline = lines.filter((line) => line.text === '\\ No newline at end of file');

    expect(removed.map(({ path, oldLine, newLine, text }) => ({ path, oldLine, newLine, text }))).toEqual([
      { path: 'src/one.ts', oldLine: 2, newLine: undefined, text: 'const gone = "<old>";' },
      { path: 'src/one.ts', oldLine: 10, newLine: undefined, text: 'before' },
    ]);
    expect(added.map(({ path, oldLine, newLine, text }) => ({ path, oldLine, newLine, text }))).toEqual([
      { path: 'src/one.ts', oldLine: undefined, newLine: 2, text: 'const added = "<new>";' },
      { path: 'src/one.ts', oldLine: undefined, newLine: 11, text: 'after' },
      { path: 'src/new.ts', oldLine: undefined, newLine: 1, text: 'export const one = 1;' },
      { path: 'src/new.ts', oldLine: undefined, newLine: 2, text: 'export const two = 2;' },
      { path: 'src/new.ts', oldLine: undefined, newLine: 3, text: 'export const three = 3;' },
    ]);
    expect(context.map(({ oldLine, newLine, text }) => ({ oldLine, newLine, text }))).toEqual([
      { oldLine: 1, newLine: 1, text: 'const old = 1;' },
      { oldLine: 3, newLine: 3, text: 'const kept = true;' },
      { oldLine: 11, newLine: 12, text: 'same' },
    ]);
    expect(noNewline).toHaveLength(2);
    expect(lines.filter((line) => ['meta', 'hunk'].includes(line.kind)).every((line) => line.oldLine === undefined && line.newLine === undefined)).toBe(true);
  });

  test('counts source lines that resemble file headers and keeps the no-newline marker', () => {
    const { lines, counts } = parseUnifiedDiff([
      '--- a/example.txt',
      '+++ b/example.txt',
      '@@ -1 +1,2 @@',
      '--- old text',
      '+++ new text',
      '+index text',
      '\\ No newline at end of file',
    ].join('\n'), 'example.txt');

    expect(counts).toEqual({ added: 2, removed: 1 });
    expect(lines.map((line) => line.text)).toEqual([
      '@@ -1 +1,2 @@', '-- old text', '++ new text', 'index text', '\\ No newline at end of file',
    ]);
  });

  test('pairs remove and add runs in order and leaves unmatched additions on the right', () => {
    const line = (kind: ReviewLine['kind'], text: string, oldLine?: number, newLine?: number): ReviewLine => ({
      kind,
      text,
      html: text,
      path: 'src/example.ts',
      oldLine,
      newLine,
    });
    const removedOne = line('remove', 'old one', 4);
    const removedTwo = line('remove', 'old two', 5);
    const addedOne = line('add', 'new one', undefined, 4);
    const addedTwo = line('add', 'new two', undefined, 5);
    const addedThree = line('add', 'new three', undefined, 6);
    const context = line('context', 'same', 6, 7);
    const hunk = line('hunk', '@@ -4,3 +4,4 @@');

    const rows = buildSplitRows([hunk, removedOne, removedTwo, addedOne, addedTwo, addedThree, context]);

    expect(rows).toEqual([
      { index: 0, left: hunk },
      { index: 1, left: removedOne, right: addedOne },
      { index: 2, left: removedTwo, right: addedTwo },
      { index: 3, right: addedThree },
      { index: 4, left: context, right: context },
    ]);
  });

  test('anchors both diff layouts to new-file lines, including deleted rows', () => {
    const diff = [
      'diff --git a/demo.ts b/demo.ts',
      '--- a/demo.ts',
      '+++ b/demo.ts',
      '@@ -5,6 +5,5 @@',
      ' kept5',
      '-removed6',
      '-removed7',
      '+replacement6',
      ' kept7',
      ' kept8',
      ' kept9',
      '@@ -20,2 +19,1 @@',
      ' kept19',
      '-last21',
    ].join('\n');
    const { lines } = parseUnifiedDiff(diff, 'demo.ts');
    const anchors = newLineAnchors(lines);
    const source = lines.filter((line) => line.kind === 'context' || line.kind === 'add' || line.kind === 'remove');
    expect(source.map((line) => anchors.get(line))).toEqual([5, 6, 6, 6, 7, 8, 9, 19, 19]);
    const rows = buildSplitRows(lines).filter((row) => row.left?.kind === 'remove');
    expect(rows.map((row) => anchors.get(row.right ?? row.left!))).toEqual([6, 6, 19]);
    expect([...anchors.values()].every((line) => Number.isInteger(line) && line > 0)).toBe(true);
  });

  test('numbers plain file lines and escapes source markup in highlighted HTML', () => {
    const lines = numberedFileLines('<tag>\nconst count = 2;', 'src/example.ts');

    expect(lines.map(({ kind, oldLine, newLine, text }) => ({ kind, oldLine, newLine, text }))).toEqual([
      { kind: 'context', oldLine: undefined, newLine: 1, text: '<tag>' },
      { kind: 'context', oldLine: undefined, newLine: 2, text: 'const count = 2;' },
    ]);
    expect(lines[0]?.html).toContain('&lt;tag&gt;');
    expect(lines[0]?.html).not.toContain('<tag>');
    expect(lines[1]?.html).toContain('tok-keyword');
  });

  test('drops a leading byte-order mark before numbering', () => {
    const lines = numberedFileLines('\uFEFFfirst\nsecond', 'src/example.ts');

    expect(lines.map(({ newLine, text }) => ({ newLine, text })).slice(0, 2)).toEqual([
      { newLine: 1, text: 'first' },
      { newLine: 2, text: 'second' },
    ]);
  });
});
