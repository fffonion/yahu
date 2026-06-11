const PLACEHOLDER_OPEN = '\uE000';
const PLACEHOLDER_CLOSE = '\uE001';

export function escapeHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function safeHref(value: string) {
  const href = value.trim();
  if (/^(https?:|mailto:|#|\/)/i.test(href)) return href.replace(/"/g, '&quot;');
  return '#';
}

function applyInlineFormatting(escaped: string) {
  return escaped
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/__([^_]+)__/g, '<strong>$1</strong>')
    .replace(/~~([^~]+)~~/g, '<del>$1</del>')
    .replace(/(^|[^*])\*([^*\n]+)\*(?!\*)/g, '$1<em>$2</em>')
    .replace(/(^|[^_])_([^_\n]+)_(?!_)/g, '$1<em>$2</em>');
}

function placeholder(index: number) {
  return `${PLACEHOLDER_OPEN}${index}${PLACEHOLDER_CLOSE}`;
}

export function markdownInline(text: string) {
  const tokens: string[] = [];
  const withCode = text.replace(/`([^`\n]+)`/g, (_match, code) => {
    const id = tokens.push(`<code>${escapeHtml(code)}</code>`) - 1;
    return placeholder(id);
  });
  const withLinks = withCode.replace(/\[([^\]\n]+)\]\(([^)\s]+)\)/g, (_match, label, href) => {
    const id = tokens.push(`<a href="${safeHref(href)}" target="_blank" rel="noreferrer">${applyInlineFormatting(escapeHtml(label))}</a>`) - 1;
    return placeholder(id);
  });
  let html = applyInlineFormatting(escapeHtml(withLinks));
  tokens.forEach((token, index) => {
    html = html.replaceAll(placeholder(index), token);
  });
  return html;
}

export function markdownText(text: string) {
  const lines = String(text || '').replace(/\r\n?/g, '\n').split('\n');
  const out: string[] = [];
  let paragraph: string[] = [];
  let listType: 'ul' | 'ol' | '' = '';
  let listItems: string[] = [];
  let quoteLines: string[] = [];

  const flushParagraph = () => {
    if (!paragraph.length) return;
    out.push(`<p>${paragraph.map(markdownInline).join('<br/>')}</p>`);
    paragraph = [];
  };
  const flushList = () => {
    if (!listType || !listItems.length) return;
    out.push(`<${listType}>${listItems.map((item) => `<li>${markdownInline(item)}</li>`).join('')}</${listType}>`);
    listType = '';
    listItems = [];
  };
  const flushQuote = () => {
    if (!quoteLines.length) return;
    out.push(`<blockquote>${quoteLines.map(markdownInline).join('<br/>')}</blockquote>`);
    quoteLines = [];
  };
  const flushLoose = () => { flushParagraph(); flushList(); flushQuote(); };

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const trimmed = line.trim();

    if (/^```/.test(trimmed)) {
      flushLoose();
      const codeLines: string[] = [];
      i += 1;
      while (i < lines.length && !/^```/.test(lines[i].trim())) {
        codeLines.push(lines[i]);
        i += 1;
      }
      out.push(`<pre><code>${escapeHtml(codeLines.join('\n'))}</code></pre>`);
      continue;
    }

    if (!trimmed) {
      flushLoose();
      continue;
    }

    const heading = trimmed.match(/^(#{1,6})\s+(.+)$/);
    if (heading) {
      flushLoose();
      const level = heading[1].length;
      out.push(`<h${level}>${markdownInline(heading[2].trim())}</h${level}>`);
      continue;
    }

    if (/^(?:-{3,}|\*{3,}|_{3,})$/.test(trimmed)) {
      flushLoose();
      out.push('<hr/>');
      continue;
    }

    const quote = line.match(/^>\s?(.*)$/);
    if (quote) {
      flushParagraph();
      flushList();
      quoteLines.push(quote[1]);
      continue;
    }

    const unordered = line.match(/^\s*[-*+]\s+(.+)$/);
    const ordered = line.match(/^\s*\d+[.)]\s+(.+)$/);
    if (unordered || ordered) {
      flushParagraph();
      flushQuote();
      const nextType = unordered ? 'ul' : 'ol';
      if (listType && listType !== nextType) flushList();
      listType = nextType;
      listItems.push((unordered || ordered)?.[1] || '');
      continue;
    }

    flushList();
    flushQuote();
    paragraph.push(line);
  }

  flushLoose();
  return out.join('');
}
