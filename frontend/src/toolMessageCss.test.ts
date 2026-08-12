import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => [readFileSync(new URL('./App.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8'), readFileSync(new URL('./chatMessage.ts', import.meta.url), 'utf8')].join('\n');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('tool message structured layout css', () => {
  test('tool structured keys use a fixed 8 character column', () => {
    const styles = css();
    expect(styles).toContain('.tool-field{display:grid;grid-template-columns:8ch minmax(0,1fr)');
    expect(styles).toContain('.tool-key{width:8ch');
  });

  test('tool messages keep API-provided names for icon mapping', () => {
    const source = app();
    expect(source).toContain('toolName: rawToolName(raw)');
    expect(source).toContain('summarizeToolMessage(message.content, message.toolName, message.toolInput)');
  });

  test('tool detail renders invocation and result sections', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('toolInput: rawToolInput(raw)');
    expect(source).toContain("title={t('tool.invocation')}");
    expect(source).toContain("title={t('tool.result')}");
    expect(styles).toContain('.tool-detail-section{display:grid;gap:7px}');
  });

  test('patch and read_file results render filename-aware highlighted code blocks', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("canonicalToolName === 'read_file'");
    expect(source).toContain('highlightedReadFileLines(summary.result, summary.filePath)');
    expect(source).toContain("canonicalToolName === 'patch'");
    expect(source).toContain('highlightedDiffLines(diff, filePath)');
    expect(source).toContain('tool-code-line-number');
    expect(styles).toContain('.tool-code-line.diff-add{background:');
    expect(styles).toContain('.tool-code-line.diff-remove{background:');
    expect(styles).toContain('.tool-code-source .tok-keyword{color:var(--syntax-keyword)}');
  });

  test('search_files results parse JSON into grouped match rows instead of recursive fields', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("canonicalToolName === 'search_files'");
    expect(source).toContain('<SearchFilesResultView value={summary.result} />');
    expect(source).toContain('parseSearchFilesResult(value)');
    expect(source).toContain('search-files-path');
    expect(source).toContain('search-files-line');
    expect(styles).toContain('.search-files-result{display:grid;gap:9px;min-width:0}');
    expect(styles).toContain('.search-files-path{');
    expect(styles).toContain('.search-files-file{');
  });

  test('assistant structured content reuses the exact recursive tool value formatter', () => {
    const source = app();
    expect(source).toContain('export function StructuredDataView({ value }: { value: unknown })');
    expect(source).toContain('<StructuredDataView value={child} />');
    expect(source).toContain('<StructuredDataView value={value} />');
    expect(source).toContain('message.structuredContent ? <StructuredDataView value={message.structuredContent.value} />');
  });

  test('tool summaries keep the icon inside the card with compact typography', () => {
    const source = app();
    const styles = css();
    expect(source).not.toContain('<div className="avatar">{getToolIcon(toolName)}</div>');
    expect(source).toContain('<span className={`tool-inline-icon tool-icon-${toolIconTone(toolName)}`}>{getToolIcon(toolName)}</span>');
    expect(styles).toContain('.msg-row.tool{grid-template-columns:minmax(0,1fr);max-width:920px;align-items:start}');
    expect(styles).toContain('.tool-summary{width:100%;border:0;background:transparent;color:var(--text);display:grid;grid-template-columns:18px max-content minmax(0,1fr) 20px;gap:1ch;');
    expect(styles).toContain('.tool-inline-icon{display:grid;color:var(--accent);place-items:center;border-radius:5px;padding:2px;background:color-mix(in srgb,currentColor 13%,transparent)}');
    expect(styles).toContain('.tool-inline-icon.tool-icon-terminal{color:var(--tool-terminal)}');
    expect(styles).toContain('.tool-inline-icon.tool-icon-file{color:var(--tool-file)}');
    expect(styles).toContain('.tool-inline-icon.tool-icon-search{color:var(--tool-search)}');
    expect(styles).toContain('.tool-title{font-size:12px;font-weight:500;color:var(--accent);white-space:nowrap}');
    expect(styles).toContain('.tool-subtitle{font-size:12px;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}');
  });

  test('mobile tool text keeps the desktop semantic colors', () => {
    const styles = css();
    expect(styles).toContain('.mobile-compact-chat .tool-summary{color:var(--text)}');
    expect(styles).toContain('.mobile-compact-chat .tool-title{color:var(--accent)}');
    expect(styles).toContain('.mobile-compact-chat .tool-subtitle{color:var(--muted)}');
    expect(styles).toContain('.mobile-compact-chat .tool-key,.mobile-compact-chat .tool-detail-section h4{color:var(--muted)}');
    expect(styles).toContain('.mobile-compact-chat .tool-scalar.string{color:var(--text)}');
    expect(styles).toContain('.mobile-compact-chat .tool-scalar.number,.mobile-compact-chat .tool-scalar.boolean{color:var(--accent-2)}');
  });
});
