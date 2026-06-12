import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat context window meter', () => {
  test('chat header renders an API-token context window bar before the settings control', () => {
    const source = app();
    expect(source).toContain('type ChatMessage = { id: string; role: Role; content: string; reasoning?: string; timestamp?: string | number; pending?: boolean; toolName?: string; toolInput?: unknown; toolCalls?: unknown; tokenCount?: number; model?: string; provider?: string; platformSenderName?: string; platformSenderId?: string }');
    expect(source).toContain('function ContextWindowMeter({ used, total, approximate = false }: { used?: number; total?: number; approximate?: boolean })');
    expect(source).toContain('const contextWindowTotal = currentModelOption?.contextLength || fallbackContextWindowForModel(currentModel);');
    expect(source).toContain('const contextWindowUsage = contextWindowTokens(props.messages, props.input, props.attachments, props.hasOlder || props.hasNewer, props.contextWindowSnapshot?.sessionId === props.activeSessionId ? props.contextWindowSnapshot : undefined);');
    expect(source).toContain('<div className="header-actions chat-header-actions">');
    expect(source).toContain('<ContextWindowMeter used={contextWindowUsage.used} approximate={contextWindowUsage.approximate} total={contextWindowTotal} />\n        <HeaderThemeControl theme={props.theme} setTheme={props.setTheme} mode={props.mode} onNavigateToSettings={props.onNavigateToSettings} />');
    expect(source).not.toContain('<ContextWindowMeter used={contextWindowUsage.used} approximate={contextWindowUsage.approximate} total={contextWindowTotal} />\n          <button className="send-btn mobile-icon-only"');
    expect(source).toContain('function estimateContextWindowTokens(messages: ChatMessage[], input: string, attachments: Attachment[]): number');
    expect(source).toContain('function roughTokenCount(text: string): number');
  });

  test('context window used value prefers API token_count and falls back to approximate frontend estimates', () => {
    const source = app();
    expect(source).toContain('type ContextWindowSnapshot = { sessionId: string; used: number; approximate?: boolean; compressed?: boolean }');
    expect(source).toContain('const [contextWindowSnapshot, setContextWindowSnapshot] = useState<ContextWindowSnapshot | null>(null);');
    expect(source).toContain('fetch(`/chat/context-window/${encodeURIComponent(sessionId)}`)');
    expect(source).toContain('const contextWindowUsage = contextWindowTokens(props.messages, props.input, props.attachments, props.hasOlder || props.hasNewer, props.contextWindowSnapshot?.sessionId === props.activeSessionId ? props.contextWindowSnapshot : undefined);');
    expect(source).toContain('const tokenCount = readTokenCount(raw);');
    expect(source).toContain('if (tokenCount !== undefined) msg.tokenCount = tokenCount;');
    expect(source).toContain('function contextWindowTokens(messages: ChatMessage[], input: string, attachments: Attachment[], hasUnloadedHistory: boolean, serverUsage?: { used?: number; approximate?: boolean }): { used: number; approximate: boolean }');
    expect(source).toContain('if (serverUsage && typeof serverUsage.used === \'number\' && Number.isFinite(serverUsage.used))');
    expect(source).toContain('if (!input.trim() && !attachments.length && !hasUnloadedHistory && messages.length && messages.every((message) => !message.pending && message.tokenCount !== undefined))');
    expect(source).toContain('return { used: exactContextWindowTokens(messages), approximate: false };');
    expect(source).toContain('return { used: estimateContextWindowTokens(messages, input, attachments), approximate: true };');
    expect(source).toContain('const label = `${safeUsed === undefined ? \'~\' : `${approximate ? \'~\' : \'\'}${formatContextTokens(safeUsed)}`} / ${formatContextTokens(safeTotal)}`;');
  });

  test('context window meter parses model context and has compact layout styling', () => {
    const source = app();
    expect(source).toContain('const contextLength = readContextLength(modelRow);');
    expect(source).toContain('contextLength ? { id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined, contextLength }');
    const styles = css();
    expect(styles).toContain('.chat-header-actions .context-window-meter{margin-left:0');
    expect(styles).toContain('.context-window-fill{height:100%;border-radius:999px;background:var(--accent)');
    expect(styles).not.toContain('.composer-wrap.composer-compact .composer-footer .context-window-meter{display:none}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .send-btn{margin-left:auto}');
    expect(styles).toContain('@media (max-width:760px){.chat-header-actions .context-window-meter{width:116px;min-width:0;max-width:116px;flex:0 1 116px}.chat-header-actions .context-window-track{min-width:30px}');
    expect(styles).toContain('.chat-header-actions .header-theme-control{margin-left:0}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact) .composer-footer .send-btn{margin-left:auto}');
  });
});
