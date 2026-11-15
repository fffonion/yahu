import { describe, expect, test } from 'bun:test';
import type { ReviewState } from './reviewComments';
import { buildReviewReport, reviewFileName, reviewReportPrompt } from './reviewReport';

describe('review report helpers', () => {
  test('renders files and comments in deterministic path, line, and timestamp order', () => {
    const state: ReviewState = {
      id: 'rv_test',
      createdAt: 1,
      files: ['z.ts', 'a.ts'],
      comments: [
        { id: 'z', path: 'z.ts', anchor: { side: 'old', line: 9 }, body: 'last file', createdAt: 100, updatedAt: 100 },
        { id: 'a-late', path: 'a.ts', anchor: { side: 'new', line: 1 }, body: 'later note', createdAt: 30, updatedAt: 30 },
        { id: 'a-line4', path: 'a.ts', anchor: { side: 'old', line: 4 }, body: 'fourth line', createdAt: 40, updatedAt: 40 },
        { id: 'a-early', path: 'a.ts', anchor: { side: 'new', line: 1 }, body: 'earlier note', createdAt: 20, updatedAt: 20 },
      ],
    };
    const report = buildReviewReport(state, {
      workspace: '/work/project',
      generatedAt: Date.UTC(2026, 1, 3, 4, 5, 6),
    });

    expect(report).toContain('# Code Review Report');
    expect(report).toContain('2026-02-03T04:05:06.000Z');
    expect(report).toContain('/work/project');
    expect(report).toContain('Files: 2');
    expect(report).toContain('Comments: 4');
    expect(report).toContain('- line 1 (new): earlier note');
    expect(report).toContain('- line 1 (new): later note');
    expect(report).toContain('- line 4 (old): fourth line');
    expect(report).toContain('- line 9 (old): last file');
    expect(report.indexOf('## a.ts')).toBeLessThan(report.indexOf('## z.ts'));
    expect(report.indexOf('earlier note')).toBeLessThan(report.indexOf('later note'));
    expect(report.indexOf('later note')).toBeLessThan(report.indexOf('fourth line'));
  });

  test('includes numbered current-file context around each new-line comment', () => {
    const source = Array.from({ length: 9 }, (_, index) => `code ${index + 1}`).join('\n');
    const state: ReviewState = {
      id: 'rv_context', createdAt: 1, files: ['a.ts'],
      comments: [
        { id: 'middle', path: 'a.ts', anchor: { side: 'new', line: 5 }, body: 'check this', createdAt: 1, updatedAt: 1 },
        { id: 'edge', path: 'a.ts', anchor: { side: 'new', line: 1 }, body: 'edge', createdAt: 2, updatedAt: 2 },
      ],
    };
    const report = buildReviewReport(state, { workspace: '/work', generatedAt: 0, contents: new Map([['a.ts', source]]) });
    expect(report).toContain('- line 5 (new): check this\n\n    2 | code 2\n    3 | code 3\n    4 | code 4\n    5 | code 5\n    6 | code 6\n    7 | code 7\n    8 | code 8');
    expect(report).toContain('- line 1 (new): edge\n\n    1 | code 1\n    2 | code 2\n    3 | code 3\n    4 | code 4');
    expect(report).not.toContain('    0 |');
  });

  test('returns the exact file-specific report prompt', () => {
    expect(reviewReportPrompt('/work/project/review.md')).toBe('用户做了代码审查，结论在/work/project/review.md中，请解决');
  });

  test('formats the local date and time with leading zeroes in the file name', () => {
    const localTimestamp = new Date(2026, 0, 2, 3, 4, 5).getTime();
    expect(reviewFileName(localTimestamp)).toBe('review-20260102-030405.md');
  });
});
