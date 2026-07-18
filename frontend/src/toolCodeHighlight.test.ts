import { describe, expect, test } from 'bun:test';
import { highlightedDiffLines, highlightedReadFileLines } from './toolCodeHighlight';

describe('tool code result highlighting', () => {
  test('separates read_file line numbers from syntax-highlighted source', () => {
    const lines = highlightedReadFileLines(
      '12|export const answer: number = 42;\n13|// retained comment',
      '/home/wow/project/src/answer.ts',
    );

    expect(lines).toHaveLength(2);
    expect(lines[0].lineNumber).toBe('12');
    expect(lines[0].html).not.toContain('12|');
    expect(lines[0].html).toContain('<span class="tok-keyword">export</span>');
    expect(lines[0].html).toContain('<span class="tok-keyword">const</span>');
    expect(lines[0].html).toContain('<span class="tok-number">42</span>');
    expect(lines[1].lineNumber).toBe('13');
    expect(lines[1].html).toContain('<span class="tok-comment">// retained comment</span>');
  });

  test('highlights diff structure and source syntax using each target filename', () => {
    const lines = highlightedDiffLines([
      '--- a/src/answer.ts',
      '+++ b/src/answer.ts',
      '@@ -1 +1 @@',
      '-const answer = "old";',
      '+export const answer = 42;',
      '--- a/scripts/check.py',
      '+++ b/scripts/check.py',
      '@@ -1 +1 @@',
      '-pass',
      '+def check():',
    ].join('\n'), '/tmp/fallback.txt');

    expect(lines[0].kind).toBe('file');
    expect(lines[2].kind).toBe('hunk');
    expect(lines[3]).toMatchObject({ kind: 'remove', prefix: '-', filePath: 'src/answer.ts' });
    expect(lines[4]).toMatchObject({ kind: 'add', prefix: '+', filePath: 'src/answer.ts' });
    expect(lines[4].html).toContain('<span class="tok-keyword">export</span>');
    expect(lines[4].html).toContain('<span class="tok-number">42</span>');
    expect(lines[9]).toMatchObject({ kind: 'add', prefix: '+', filePath: 'scripts/check.py' });
    expect(lines[9].html).toContain('<span class="tok-keyword">def</span>');
  });

  test('uses JSON filename syntax for read_file values and leaves plain text escaped', () => {
    const json = highlightedReadFileLines('1|{"enabled": true, "count": 3}', '/tmp/config.json')[0].html;
    expect(json).toContain('<span class="tok-keyword">"enabled":</span>');
    expect(json).toContain('<span class="tok-keyword">true</span>');
    expect(json).toContain('<span class="tok-number">3</span>');

    const plain = highlightedReadFileLines('1|const value = <unsafe>', '/tmp/notes.txt')[0].html;
    expect(plain).toBe('const value = &lt;unsafe&gt;');
  });
});
