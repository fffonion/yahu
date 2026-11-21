import { expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { ChatTranscript, type ChatMessage } from './ChatTranscript';

test('file tool summaries expose their full navigation path in regular and compact transcripts', () => {
  const message: ChatMessage = { id: 'file-tool', role: 'tool', content: JSON.stringify({ resolved_path: '/srv/workspace/project/main.ts', content: '1|example' }), toolName: 'read_file', toolInput: { path: 'project/main.ts' } };
  for (const compact of [false, true]) {
    const html = renderToStaticMarkup(<ChatTranscript messages={[message]} showReasoning showToolCalls streaming compact={compact} />);
    expect(html.includes('data-tool-path="/srv/workspace/project/main.ts"')).toBe(true);
    expect(html.includes('class="tool-summary"')).toBe(true);
    expect(html.includes('data-workspace-review="/srv/workspace/project/main.ts"')).toBe(true);
  }
});

test('search_files keeps workspace navigation without exposing a review button', () => {
  for (const toolName of ['search_files', 'functions.search_files']) {
    const message: ChatMessage = { id: 'search-tool', role: 'tool', content: JSON.stringify({ files: ['project/src/main.ts'] }), toolName, toolInput: { pattern: '*.ts', path: 'project/src' } };
    for (const compact of [false, true]) {
      const html = renderToStaticMarkup(<ChatTranscript messages={[message]} showReasoning showToolCalls streaming compact={compact} />);
      expect(html.includes('data-tool-path="project/src"')).toBe(true);
      expect(html.includes('data-workspace-review')).toBe(false);
    }
  }
});
