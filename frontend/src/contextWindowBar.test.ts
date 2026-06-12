import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('chat composer context window meter', () => {
  test('composer renders a context window bar immediately before send', () => {
    const source = app();
    expect(source).toContain('type ModelOption = { id: string; label: string; provider?: string; contextLength?: number }');
    expect(source).toContain('function ContextWindowMeter({ used, total }: { used: number; total: number })');
    expect(source).toContain('const contextWindowTotal = currentModelOption?.contextLength || fallbackContextWindowForModel(currentModel);');
    expect(source).toContain('const contextWindowUsed = estimateContextWindowTokens(visibleMessages, props.input, props.attachments);');
    expect(source).toContain('<ContextWindowMeter used={contextWindowUsed} total={contextWindowTotal} />\n          <button className="send-btn mobile-icon-only"');
  });

  test('context window meter parses model context and has compact layout styling', () => {
    const source = app();
    expect(source).toContain('const contextLength = readContextLength(modelRow);');
    expect(source).toContain('contextLength ? { id: modelId, label: String(label || (providerName ? `${providerName} · ${modelId}` : modelId)), provider: providerName || undefined, contextLength }');
    const styles = css();
    expect(styles).toContain('.context-window-meter{margin-left:auto');
    expect(styles).toContain('.context-window-fill{height:100%;border-radius:999px;background:var(--accent)');
    expect(styles).toContain('.composer-wrap.composer-compact .composer-footer .context-window-meter{display:none}');
    expect(styles).toContain('@media (max-width:760px){.context-window-meter{max-width:120px');
  });
});
