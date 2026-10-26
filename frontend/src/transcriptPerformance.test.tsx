import React from 'react';
import { describe, expect, test } from 'bun:test';
import { renderToStaticMarkup } from 'react-dom/server';
import { ChatTranscript, type ChatMessage } from './ChatTranscript';

const tool: ChatMessage = { id: 'tool', role: 'tool', toolName: 'terminal', content: JSON.stringify({ tool_name: 'terminal', output: 'Unopened expensive result', exit_code: 0 }) };

describe('transcript rendering cost', () => {
  test('keeps streaming and explicitly opened details mounted immediately', () => {
    const live = renderToStaticMarkup(<ChatTranscript messages={[tool]} showReasoning showToolCalls streaming />);
    const forced = renderToStaticMarkup(<ChatTranscript messages={[{ id: 'u', role: 'user', content: 'Task' }, tool]} showReasoning showToolCalls forceOpenLatestDetailToken={1} />);
    for (const html of [live, forced]) {
      expect(html).toContain('tool-card');
      expect(html).toContain('open=""');
    }
  });
  test('does not mount unopened completed tool details in either layout', () => {
    for (const compact of [false, true]) {
      const html = renderToStaticMarkup(<ChatTranscript messages={[{ id: 'user', role: 'user', content: 'Task' }, tool, { id: 'final', role: 'assistant', content: 'Completed' }]} compact={compact} showReasoning showToolCalls />);
      expect(html).toContain('turn-detail-summary');
      expect(html).toContain('Completed');
      expect(html).not.toContain('tool-card');
    }
  });
});
