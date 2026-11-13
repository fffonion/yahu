import { describe, expect, test } from 'bun:test';
import { buildSplitRows, numberedFileLines, parseUnifiedDiff, type ReviewLine } from './reviewDiff';

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

    const lines = parseUnifiedDiff(diff, 'fallback.ts');
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
    expect(lines.filter((line) => ['file', 'meta', 'hunk'].includes(line.kind)).every((line) => line.oldLine === undefined && line.newLine === undefined)).toBe(true);
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
});
