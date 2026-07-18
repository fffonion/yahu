export function escapeHighlightedHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

export function languageForFilePath(filePath?: string) {
  const ext = (filePath || '').split('.').pop()?.toLowerCase();
  switch (ext) {
    case 'rs': return 'rust';
    case 'py': return 'python';
    case 'ts': case 'tsx': case 'js': case 'jsx': return 'javascript';
    case 'css': return 'css';
    case 'html': case 'htm': return 'html';
    case 'json': return 'json';
    case 'md': return 'markdown';
    case 'toml': case 'yaml': case 'yml': return 'config';
    case 'sh': case 'bash': return 'shell';
    case 'go': return 'go';
    case 'c': case 'h': return 'c';
    case 'cpp': case 'hpp': case 'cc': case 'cxx': return 'cpp';
    case 'java': return 'java';
    case 'rb': return 'ruby';
    case 'sql': return 'sql';
    case 'xml': case 'svg': return 'xml';
    default: return 'plain';
  }
}

const patterns: Record<string, RegExp> = {
  rust: /\/\/[^\n]*|\/\/!.*|"(?:[^"\\]|\\.)*"|r#"(?:[^"]|"(?!\n#))*"#|r"(?:[^"]|"(?!\n))*"|'(?:[^'\\]|\\.)'|b'(?:[^'\\])'|\b(?:fn|let|mut|pub|struct|impl|trait|enum|match|if|else|for|while|loop|return|use|mod|const|static|type|where|as|in|ref|move|unsafe|extern|crate|self|super|true|false|Some|None|Ok|Err)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?(?:_?\d+)*(?:[uUiIfF](?:8|16|32|64|128|size)?)?\b/g,
  python: /#[^\n]*|"""["\n]*?"""|'''['\n]*?'''|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:def|class|import|from|return|if|elif|else|for|while|try|except|finally|raise|with|as|in|is|not|and|or|lambda|yield|async|await|pass|break|continue|True|False|None)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
  javascript: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|`(?:[^`\\]|\\.)*`|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:const|let|var|function|return|async|await|import|export|from|type|interface|if|else|for|while|class|extends|new|true|false|null|undefined|try|catch|throw|switch|case|default|break|continue|of|in|typeof)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
  css: /\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|#[0-9a-fA-F]{3,8}\b|\b\d+(?:\.\d+)?(?:px|em|rem|%|vh|vw|pt|ms|s|deg)?\b|[\w-]+(?=\s*:)|[:;{},]/g,
  html: /<!--[\s\S]*?-->|<[\/!]?\w[\w-]*(?:\s[^>]*)?>|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g,
  json: /"(?:[^"\\]|\\.)*"\s*:?|\b(?:true|false|null)\b|-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
  markdown: /^#{1,6}\s.*$|`[^`]+`|```[\s\S]*?```|\*\*[^*]+\*\*|__[^_]+__|\[[^\]]+\]\([^)]+\)/gm,
  config: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:true|false|null|yes|no|on|off)\b|\b\d+(?:\.\d+)?\b/g,
  shell: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|in|function|return|exit|echo|export|source|local|readonly|declare)\b|\b\d+\b|\$\{?[\w_]+\}?/g,
  go: /\/\/[^\n]*|"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`|'(?:[^'\\])'|\b(?:func|return|if|else|for|range|switch|case|default|break|continue|go|defer|chan|select|map|struct|interface|type|var|const|package|import|nil|true|false)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b/g,
  c: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:if|else|for|while|do|switch|case|default|break|continue|return|struct|typedef|enum|union|static|extern|const|volatile|sizeof|void|int|char|float|double|long|short|unsigned|signed)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFlLuU]?\b/g,
  cpp: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:class|struct|enum|namespace|using|template|typename|virtual|override|public|private|protected|const|constexpr|auto|decltype|static_cast|dynamic_cast|nullptr|new|delete|try|catch|throw|if|else|for|while|do|switch|case|default|break|continue|return|true|false)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFlLuU]?\b/g,
  java: /\/\/[^\n]*|\/\*[\s\S]*?\*\/|"(?:[^"\\]|\\.)*"|'(?:[^'\\])'|\b(?:public|private|protected|static|final|class|interface|extends|implements|abstract|synchronized|volatile|transient|native|strictfp|void|int|long|double|float|boolean|char|byte|short|new|return|if|else|for|while|do|switch|case|default|break|continue|try|catch|throw|throws|import|package|true|false|null|this|super)\b|\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?[fFdDlL]?\b/g,
  ruby: /#[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|:(?:\w+[?!]?)|:\s*"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`|\b(?:def|end|class|module|if|elsif|else|unless|while|until|for|do|begin|rescue|ensure|case|when|return|yield|self|nil|true|false|and|or|not|require|include|extend|attr_accessor|attr_reader|attr_writer|private|protected|public)\b|\b\d+(?:\.\d+)?\b/g,
  sql: /--[^\n]*|\/\*[\s\S]*?\*\/|'(?:[^'\\]|\\.)*'|\b(?:SELECT|FROM|WHERE|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|TABLE|INDEX|VIEW|JOIN|LEFT|RIGHT|INNER|OUTER|ON|AS|AND|OR|NOT|IN|BETWEEN|LIKE|IS|NULL|ORDER|BY|GROUP|HAVING|LIMIT|OFFSET|UNION|ALL|DISTINCT|COUNT|SUM|AVG|MIN|MAX|CASE|WHEN|THEN|ELSE|END|SET|VALUES|INTO|PRIMARY|KEY|FOREIGN|REFERENCES|CONSTRAINT|DEFAULT|CHECK|UNIQUE|EXISTS)\b|\b\d+(?:\.\d+)?\b/gi,
  xml: /<!--[\s\S]*?-->|<[\/!]?\w[\w:.-]*(?:\s[^>]*)?>|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'/g,
};

export function highlightSourceText(text: string, filePath?: string) {
  const language = languageForFilePath(filePath);
  if (language === 'plain') return escapeHighlightedHtml(text);
  const token = patterns[language];

  let out = '';
  let last = 0;
  for (const match of text.matchAll(token)) {
    const part = match[0];
    const index = match.index || 0;
    out += escapeHighlightedHtml(text.slice(last, index));
    let cls: string;
    if (language === 'css') {
      if (part.startsWith('/*')) cls = 'tok-comment';
      else if (part.startsWith('#') || (part.startsWith('"') && part.includes(':')) || part.startsWith("'")) cls = 'tok-string';
      else if (/^[\w-]+(?=:)/.test(part)) cls = 'tok-keyword';
      else if (/[:;{},]/.test(part)) cls = 'tok-keyword';
      else if (/^\d/.test(part)) cls = 'tok-number';
      else cls = 'tok-string';
    } else if (language === 'html' || language === 'xml') {
      if (part.startsWith('<!--')) cls = 'tok-comment';
      else if (part.startsWith('<')) cls = 'tok-keyword';
      else cls = 'tok-string';
    } else if (language === 'json') {
      if (part.trimEnd().endsWith(':')) cls = 'tok-keyword';
      else if (part.startsWith('"')) cls = 'tok-string';
      else if (/^-?\d/.test(part)) cls = 'tok-number';
      else cls = 'tok-keyword';
    } else if (language === 'markdown') {
      if (part.startsWith('#')) cls = 'tok-keyword';
      else cls = 'tok-string';
    } else {
      if (part.startsWith('//') || part.startsWith('/*') || part.startsWith('#') || part.startsWith('--') || part.startsWith('<!--')) cls = 'tok-comment';
      else if (part.startsWith('"') || part.startsWith("'") || part.startsWith('`') || part.startsWith('r"') || part.startsWith('r#"') || part.startsWith('b\'')) cls = 'tok-string';
      else if (/^\d/.test(part)) cls = 'tok-number';
      else cls = 'tok-keyword';
    }
    out += `<span class="${cls}">${escapeHighlightedHtml(part)}</span>`;
    last = index + part.length;
  }
  return out + escapeHighlightedHtml(text.slice(last));
}
