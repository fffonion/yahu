import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat composer context window meter', () => {
  test('composer renders an API-token context window bar immediately before send', () => {
    const source = app();
    expect(source).toContain('type ChatMessage = { id: string; role: Role; content: string; reasoning?: string; timestamp?: string | number; pending?: boolean; toolName?: string; toolInput?: unknown; toolCalls?: unknown; tokenCount?: number; model?: string; provider?: string; platformSenderName?: string; platformSenderId?: string }');
    expect(source).toContain('function ContextWindowMeter({ used, total }: { used?: number; total?: number })');
    expect(source).toContain('const contextWindowTotal = currentModelOption?.contextLength || fallbackContextWindowForModel(currentModel);');
    expect(source).toContain('const contextWindowUsed = exactContextWindowTokens(props.messages, props.input, props.attachments, props.hasOlder || props.hasNewer);');
    expect(source).toContain('<ContextWindowMeter used={contextWindowUsed} total={contextWindowTotal} />\n          <button className="send-btn mobile-icon-only"');
    expect(source).not.toContain('function estimateContextWindowTokens');
    expect(source).not.toContain('roughTokenCount');
  });

  test('context window used value is sourced from API message token_count only', () => {
    const source = app();
    expect(source).toContain('const tokenCount = readTokenCount(raw);');
    expect(source).toContain('if (tokenCount !== undefined) msg.tokenCount = tokenCount;');
    expect(source).toContain('function exactContextWindowTokens(messages: ChatMessage[], input: string, attachments: Attachment[], hasUnloadedHistory: boolean): number | undefined');
    expect(source).toContain('if (input.trim() || attachments.length || hasUnloadedHistory) return undefined;');
    expect(source).toContain('if (!messages.length || messages.some((message) => message.pending || message.tokenCount === undefined)) return undefined;');
  });

  test('context window meter parses model context and has compact layout styling', () => {
    const source = app();
    expect(source).toContain('const contextLength = readContextLength(modelRow);');
    expect(source).toContain('contextLength ? { id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined, contextLength }');
    const styles = css();
    expect(styles).toContain('.context-window-meter{margin-left:auto');
    expect(styles).toContain('.context-window-fill{height:100%;border-radius:999px;background:var(--accent)');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .context-window-meter{display:none}');
    expect(styles).toContain('.composer-footer .send-btn{margin-left:0;flex:0 0 auto}');
    expect(styles).toContain('@media (max-width:760px){.composer-wrap:not(.composer-compact) .context-window-meter{position:absolute;right:10px;top:2px;width:120px;min-width:0;max-width:120px;margin-left:0;pointer-events:none}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .send-btn{margin-left:auto}');
  });
});
