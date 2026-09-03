import { describe, expect, test } from 'bun:test';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { StructuredDataView } from './ChatTranscript';

function render(value: unknown) {
  return renderToStaticMarkup(React.createElement(StructuredDataView, { value }));
}

describe('structured tool data arrays', () => {
  test('collapses a nested array longer than three items by default', () => {
    const html = render({ delegation: { subagents: ['a', 'b', 'c', 'd'] } });

    expect(html).toContain('class="tool-array-fold"');
    expect(html).toContain('Show 4 items');
    expect(html).not.toContain('<details open');
  });

  test('leaves arrays of three items expanded inline', () => {
    const html = render({ values: [1, 2, 3] });

    expect(html).not.toContain('tool-array-fold');
    expect(html).toContain('class="tool-key">0</span>');
    expect(html).toContain('class="tool-key">2</span>');
  });

  test('leaves a top-level array unchanged', () => {
    const html = render([1, 2, 3, 4]);

    expect(html).not.toContain('tool-array-fold');
    expect(html).toContain('class="tool-key">3</span>');
  });
});
