import { describe, expect, test } from 'bun:test';
import { highlightSourceText, languageForFilePath } from './syntaxHighlight';

describe('Cap’n Proto syntax highlighting', () => {
  test('recognizes schema paths regardless of extension case', () => {
    expect(languageForFilePath('/schemas/message.CAPNP')).toBe('capnp');
  });

  test('colors schema declarations, imports, identifiers, strings and comments without interpreting HTML', () => {
    const source = '@0xdeadbeef;\nusing Shared = import "../shared.capnp";\nstruct Message { name @0 :Text; }\n# <script> comment\n';
    const html = highlightSourceText(source, 'message.capnp');
    expect(html).toContain('<span class="tok-number">@0xdeadbeef</span>');
    expect(html).toContain('<span class="tok-keyword">using</span>');
    expect(html).toContain('<span class="tok-keyword">import</span>');
    expect(html).toContain('<span class="tok-keyword">struct</span>');
    expect(html).toContain('<span class="tok-keyword">Text</span>');
    expect(html).toContain('<span class="tok-number">@0</span>');
    expect(html).toContain('<span class="tok-string">"../shared.capnp"</span>');
    expect(html).toContain('<span class="tok-comment"># &lt;script&gt; comment</span>');
    expect(html).not.toContain('<script>');
  });
});
