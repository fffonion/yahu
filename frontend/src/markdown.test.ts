import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { isMarkdownPath, markdownText } from './markdown';

const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('markdown helpers', () => {
  test('detects markdown file paths used by workspace and skills previews', () => {
    expect(isMarkdownPath('README.md')).toBe(true);
    expect(isMarkdownPath('docs/guide.markdown')).toBe(true);
    expect(isMarkdownPath('references/SKILL.MD')).toBe(true);
    expect(isMarkdownPath('src/App.tsx')).toBe(false);
    expect(isMarkdownPath('notes.md.bak')).toBe(false);
  });
});

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

  test('renders github-style markdown tables safely', () => {
    const html = markdownText(`Before

| Name | Value |
| --- | ---: |
| **Input** | 123 |
| <script> | [safe](https://example.com) |

After`);

    expect(html).toContain('<div class="md-table-wrap"><table>');
    expect(html).toContain('<thead><tr><th>Name</th><th>Value</th></tr></thead>');
    expect(html).toContain('<tbody><tr><td><strong>Input</strong></td><td>123</td></tr><tr><td>&lt;script&gt;</td><td><a href="https://example.com" target="_blank" rel="noreferrer">safe</a></td></tr></tbody>');
    expect(html).toContain('<p>Before</p>');
    expect(html).toContain('<p>After</p>');
    expect(html).not.toContain('<script>');
  });

  test('styles markdown tables without widening the chat viewport', () => {
    const styles = css();
    expect(styles).toContain('.msg-body .md-table-wrap{max-width:100%;overflow-x:auto;margin:8px 0 10px;border:1px solid var(--border);border-radius:12px}');
    expect(styles).toContain('.msg-body table{width:100%;border-collapse:collapse;font-size:13px}');
    expect(styles).toContain('.msg-body th,.msg-body td{padding:7px 9px;border-bottom:1px solid var(--border);text-align:left;vertical-align:top}');
  });
});
