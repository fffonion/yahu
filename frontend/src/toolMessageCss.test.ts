import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
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
    expect(source).toContain('title="Invocation"');
    expect(source).toContain('title="Result"');
    expect(styles).toContain('.tool-detail-section{display:grid;gap:7px}');
  });

  test('tool summaries keep the icon inside the card with compact typography', () => {
    const source = app();
    const styles = css();
    expect(source).not.toContain('<div className="avatar">{getToolIcon(toolName)}</div>');
    expect(source).toContain('<span className="tool-inline-icon">{getToolIcon(toolName)}</span>');
    expect(styles).toContain('.msg-row.tool{grid-template-columns:minmax(0,1fr);max-width:920px;align-items:start}');
    expect(styles).toContain('.tool-summary{width:100%;border:0;background:transparent;color:var(--text);display:grid;grid-template-columns:18px max-content minmax(0,1fr) 20px;gap:1ch;');
    expect(styles).toContain('.tool-inline-icon{display:grid;color:var(--accent);place-items:center}');
    expect(styles).toContain('.tool-title{font-size:12px;font-weight:500;color:var(--accent);white-space:nowrap}');
    expect(styles).toContain('.tool-subtitle{font-size:12px;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}');
  });
});
