import { describe, expect, test } from 'bun:test';
import { markdownText } from './markdown';

describe('chat markdown rendering', () => {
  test('renders common markdown blocks and inline formatting safely', () => {
    const html = markdownText(`# Title

Hello **bold** and *italic* with ~~gone~~ plus \`code\` and [link](https://example.com).

- one
- two

> quoted

\`\`\`js
const x = "<tag>";
\`\`\``);

    expect(html).toContain('<h1>Title</h1>');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toContain('<em>italic</em>');
    expect(html).toContain('<del>gone</del>');
    expect(html).toContain('<code>code</code>');
    expect(html).toContain('<a href="https://example.com" target="_blank" rel="noreferrer">link</a>');
    expect(html).toContain('<ul><li>one</li><li>two</li></ul>');
    expect(html).toContain('<blockquote>quoted</blockquote>');
    expect(html).toContain('<pre><code>const x = "&lt;tag&gt;";</code></pre>');
  });

  test('escapes raw html and blocks unsafe links', () => {
    const html = markdownText('hello <script>alert(1)</script> [bad](javascript:alert(1))');
    expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(html).toContain('<a href="#" target="_blank" rel="noreferrer">bad</a>');
    expect(html).not.toContain('<script>');
    expect(html).not.toContain('javascript:alert');
  });
});
