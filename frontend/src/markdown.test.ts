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

  test('renders MEDIA and FILE directives as chat media cards outside code blocks', () => {
    const html = markdownText(`Before
MEDIA:/tmp/chart.png
MEDIA:'/tmp/demo clip.mp4'
MEDIA:/tmp/sound.ogg
FILE:/tmp/report.pdf

\`\`\`
MEDIA:/tmp/example.png
\`\`\`
After`);

    expect(html).toContain('<p>Before</p>');
    expect(html).toContain('<figure class="md-media md-media-image"><a href="/chat/media?path=%2Ftmp%2Fchart.png" target="_blank" rel="noreferrer"><img src="/chat/media?path=%2Ftmp%2Fchart.png" alt="chart.png" loading="lazy"/></a><figcaption>chart.png</figcaption></figure>');
    expect(html).toContain('<figure class="md-media md-media-video"><video controls preload="metadata" src="/chat/media?path=%2Ftmp%2Fdemo%20clip.mp4"></video><figcaption>demo clip.mp4</figcaption></figure>');
    expect(html).toContain('<figure class="md-media md-media-audio"><audio controls src="/chat/media?path=%2Ftmp%2Fsound.ogg"></audio><figcaption>sound.ogg</figcaption></figure>');
    expect(html).toContain('<p><a class="md-media-file" href="/chat/media?path=%2Ftmp%2Freport.pdf&amp;download=1" target="_blank" rel="noreferrer">report.pdf</a></p>');
    expect(html).toContain('<pre><code>MEDIA:/tmp/example.png</code></pre>');
    expect(html).toContain('<p>After</p>');
  });

  test('styles markdown tables and media without widening the chat viewport', () => {
    const styles = css();
    expect(styles).toContain('.msg-body .md-table-wrap{max-width:100%;overflow-x:auto;margin:8px 0 10px;border:1px solid var(--border);border-radius:12px}');
    expect(styles).toContain('.msg-body table{width:100%;border-collapse:collapse;font-size:13px}');
    expect(styles).toContain('.msg-body th,.msg-body td{padding:7px 9px;border-bottom:1px solid var(--border);text-align:left;vertical-align:top}');
    expect(styles).toContain('.msg-body .md-media{margin:10px 0;max-width:min(100%,560px);display:grid;gap:6px}');
    expect(styles).toContain('.msg-body .md-media img,.msg-body .md-media video{width:100%;height:auto;max-width:100%;border-radius:12px;border:1px solid var(--border);background:var(--surface);display:block}');
  });
});
