import { describe, expect, test } from 'bun:test';
import { parseSearchFilesResult } from './searchFilesResult';

describe('search_files result parsing', () => {
  test('parses grouped content matches and omits transport formatting metadata', () => {
    const parsed = parseSearchFilesResult(JSON.stringify({
      total_count: 3,
      matches_format: "path-grouped transport description",
      matches_text: "/tmp/src/App.tsx\n  7: const answer = 42;\n  8- const nearby = true;\n/tmp/src/main.rs\n  4: fn main() {}",
    }));

    expect(parsed).toEqual({
      totalCount: 3,
      groups: [
        {
          path: '/tmp/src/App.tsx',
          matches: [
            { lineNumber: '7', content: 'const answer = 42;', isMatch: true },
            { lineNumber: '8', content: 'const nearby = true;', isMatch: false },
          ],
        },
        {
          path: '/tmp/src/main.rs',
          matches: [{ lineNumber: '4', content: 'fn main() {}', isMatch: true }],
        },
      ],
      files: [],
      error: '',
    });
  });

  test('parses file-search arrays into a path list', () => {
    expect(parseSearchFilesResult({ matches: ['/tmp/a.ts', '/tmp/b.ts'] })).toEqual({
      totalCount: 2,
      groups: [],
      files: ['/tmp/a.ts', '/tmp/b.ts'],
      error: '',
    });
  });

  test('keeps explicit errors and rejects unrelated objects', () => {
    expect(parseSearchFilesResult({ total_count: 0, error: 'Search failed' })).toEqual({
      totalCount: 0,
      groups: [],
      files: [],
      error: 'Search failed',
    });
    expect(parseSearchFilesResult({ output: 'plain tool output' })).toBeNull();
  });
});
