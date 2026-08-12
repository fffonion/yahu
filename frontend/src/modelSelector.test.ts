import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');

describe('composer model selector', () => {
  test('does not seed, restore, or persist the selector with the api-server hermes-agent placeholder', () => {
    const source = app();
    expect(source).not.toContain("useState<ModelOption[]>([{ id: 'hermes-agent'");
    expect(source).not.toContain("localStorage.getItem('model') || 'hermes-agent'");
    expect(source).toContain('readStoredModel()');
    expect(source).toContain("if (stored === 'hermes-agent')");
    expect(source).toContain("localStorage.removeItem('model')");
    expect(source).toContain('flattenModelOptions(body)');
  });

  test('new chat sends selected model with the real stream turn instead of a slash-command switch', () => {
    const source = app();
    const userBubbleIndex = source.indexOf('const userMsg: ChatMessage =');
    const streamIndex = source.indexOf('const res = await fetch(`/chat/stream/${encodeURIComponent(sessionId)}`');
    expect(userBubbleIndex).toBeGreaterThan(0);
    expect(streamIndex).toBeGreaterThan(userBubbleIndex);
    expect(source).not.toContain('switchSessionModel');
    expect(source).not.toContain('/chat/model-switch');
    expect(source).not.toContain('buildChatRequestBody(`/model ');
    expect(source).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
  });

  test('selecting a model updates local session state and carries provider without patching session metadata', () => {
    const source = app();
    expect(source).toContain('const changeSessionModel = useCallback((nextModel: string, option?: ModelOption) =>');
    expect(source).toContain('const resolvedModel = realModelOrEmpty(nextModel)');
    expect(source).toContain('setSelectedModelProvider(provider)');
    expect(source).toContain('modelRef.current = resolvedModel');
    expect(source).toContain('providerRef.current = provider');
    expect(source).toContain('const nextOverrides = { ...sessionModelOverridesRef.current, [activeSessionId]: { model: resolvedModel, provider } };');
    expect(source).toContain('sessionModelOverridesRef.current = nextOverrides;');
    expect(source).toContain('setSessionModelOverrides(nextOverrides)');
    expect(source).toContain('const sessionOverride = sessionModelOverridesRef.current[sessionId]');
    expect(source).toContain('const sessionModel = sessionOverride?.model || realModelOrEmpty(modelRef.current) || createdSession?.model || activeSession?.model || activeSessionDetail?.model ||');
    expect(source).toContain('const sessionProvider = resolveModelProvider(modelsRef.current, sessionModel, sessionOverride?.provider ?? (providerRef.current || createdSession?.provider || activeSession?.provider || activeSessionDetail?.provider || \'\'));');
    expect(source).toContain('const activeSessionModelOverride = activeSessionId ? sessionModelOverrides[activeSessionId] : undefined');
    expect(source).toContain('const sessionProvider = resolveModelProvider(modelsRef.current, sessionModel, sessionOverride?.provider ?? (providerRef.current || createdSession?.provider || activeSession?.provider || activeSessionDetail?.provider || \'\'));');
    expect(source).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
    expect(source).toContain('fetch(`/chat/stream/${encodeURIComponent(sessionId)}`, { method: \'POST\'');
    expect(source).not.toContain('apiJoin(SESSION_API_BASE, `/api/sessions/${encodeURIComponent(sessionId)}/chat/stream`)');
    expect(source).toContain('setSelectedModelProvider((current) => activeProvider !== current ? activeProvider : current)');
    expect(source).toContain('}, [activeSessionId, activeSession?.model, activeSession?.provider, sessionModelOverrides]);');
    expect(source).not.toContain('if (activeModel && activeModel !== model) setModelState(activeModel);');
    expect(source).not.toContain('body: JSON.stringify({ model: resolvedModel })');
    expect(source).toContain('setModel={changeSessionModel}');
  });

  test('keeps duplicate model ids from different providers selectable', () => {
    const source = app();
    expect(source).toContain("const key = `${providerName}\\u0000${modelId}`;");
    expect(source).toContain('seen.has(key)');
    expect(source).toContain('function modelOptionKey');
    expect(source).toContain('function findModelOption');
    expect(source).toContain('resolvePreferredModelProvider as resolveModelProvider');
    expect(source).toContain('return selectModelOption(options, realModelOrEmpty(modelId), provider);');
    expect(source).toContain('valueProvider?: string');
    expect(source).toContain('findModelOption(options, value, valueProvider)');
    expect(source).toContain('key={modelOptionKey(item)}');
    expect(source).not.toContain('props.models.filter((m: ModelOption) => m.id !== currentModel)');
  });

  test('composer details keeps model and reasoning visible in one combined trigger', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="composer-details-trigger"');
    expect(source).toContain('composer-trigger-model');
    expect(source).toContain('composer-trigger-effort');
    expect(source).not.toContain('<Settings aria-hidden="true" />');
    expect(source).not.toContain('composer-toolbar-model');
    expect(source).not.toContain('composer-toolbar-effort');
    expect(source).toContain("const [page, setPage] = useState<'advanced' | 'model' | 'effort' | 'details'>('advanced');");
    expect(source).toContain('const [advancedOpen, setAdvancedOpen] = useState(false);');
    expect(source).toContain('composer-details-flyout');
    expect(source).toContain("t('chat.advanced')");
    expect(source).toContain("detailsOption(t('chat.reasoning'), showReasoning, onToggleReasoning)");
    expect(source).toContain("detailsOption(t('chat.tool'), showToolCalls, onToggleToolCalls)");
    expect(source).toContain("detailsOption(t('chat.collapseDetails'), compactMessages, onToggleCompact)");
    expect(styles).toContain('.composer-details-trigger{');
    expect(styles).toContain('.composer-details-advanced{');
    expect(styles).toContain('.composer-details-option{');
  });

  test('bottom dropdown menu renders above the composer box instead of being clipped by it', () => {
    const styles = css();
    const composerBoxRules = styles.match(/\.composer-box\{[^}]*\}/g) || [];
    const dropdownMenuRules = styles.match(/\.dropdown-menu\{[^}]*\}/g) || [];
    const zIndexes = dropdownMenuRules
      .map((rule) => Number(rule.match(/z-index:(\d+)/)?.[1] || 0))
      .filter(Boolean);
    expect(composerBoxRules.join('\n')).toContain('overflow:visible');
    expect(Math.max(...zIndexes)).toBeGreaterThanOrEqual(120);
  });

  test('mobile dropdowns escape the composer overflow and sit above bottom navigation', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("${open ? 'open' : ''}");
    expect(styles).toContain('.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow:visible;position:relative;z-index:160;padding:10px 10px calc(var(--mobile-bottom-nav-height) + env(safe-area-inset-bottom,0px))}');
    expect(styles).toContain('.composer-box,.composer-footer{overflow:visible}');
    expect(styles).toContain('.dropdown-control.open{z-index:170}');
    expect(styles).toContain('.dropdown-control.open .dropdown-menu{z-index:180}');
    expect(styles).toContain('.dropdown-control.searchable.open .dropdown-menu{position:fixed;left:12px;right:12px;bottom:calc(210px + env(safe-area-inset-bottom,0px));width:auto;min-width:0;max-width:none;max-height:min(48vh,380px);border-radius:18px;padding:12px}');
    expect(styles).not.toContain('.main-panel,.chat-header,.chat-scroll,.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow-x:hidden}');
  });

  test('effort is available from the combined settings menu', () => {
    const source = app();
    const styles = css();
    expect(source).not.toContain('composer-toolbar-effort');
    expect(source).toContain('className="composer-details-flyout"');
    expect(source).toContain("REASONING_EFFORTS.map((item) => <button type=\"button\" className=\"composer-submenu-row\"");
    expect(styles).toContain('.composer-details-flyout{');
    expect(styles).toContain('.composer-details-trigger:hover{');
  });

  test('compact reference composer removes access text and keeps the model menu right-aligned', () => {
    const source = app();
    const styles = css();
    expect(source).not.toContain('Full access');
    expect(styles).toContain('.composer-box textarea{height:64px;min-height:64px;padding:15px 16px 8px;');
    expect(styles).toContain('.composer-tools .attach-btn{width:28px;height:28px;');
    expect(styles).toContain('.composer-footer .composer-primary-btn{width:36px;height:36px;');
    expect(styles).toContain('.composer-right-controls .dropdown-control.searchable .dropdown-menu{left:auto;right:0;width:min(360px,calc(100vw - 48px))}');
  });

  test('composer model and reasoning triggers omit the model caret and brain icon', () => {
    const source = app();
    const styles = css();
    expect(source).not.toContain('<ChevronRight className="dropdown-caret" />');
    expect(source).not.toContain('<Brain aria-hidden="true" />\n      <span>{value}</span>');
    expect(source).not.toContain('<ChevronDown className="reasoning-trigger-caret" aria-hidden="true" />');
    expect(styles).not.toContain('.composer-right-controls .dropdown-caret{');
    expect(styles).not.toContain('.reasoning-trigger-caret{');
  });

  test('composer edge blends into the transcript and the reasoning handle sits inside a thicker bar', () => {
    const styles = css();
    expect(styles).toContain('.composer-box{background:transparent;border:1px solid color-mix(in srgb,var(--text) 9%,transparent);border-radius:22px;box-shadow:none}');
    expect(styles).toContain('.composer-box textarea,.composer-footer{background:transparent}');
    expect(styles).toContain('.composer-wrap:not(.composer-compact){background:transparent;box-shadow:none}.composer-wrap:not(.composer-compact) .composer-box,.composer-wrap:not(.composer-compact) .composer-box textarea{background:transparent}');
    expect(styles).toContain('.composer-wrap::before{content:"";position:absolute;left:0;right:0;top:-52px;height:76px;');
    expect(styles).toContain('.reasoning-slider::-webkit-slider-runnable-track{height:16px;');
    expect(styles).toContain('.reasoning-slider::-webkit-slider-thumb{width:10px;height:10px;margin-top:0;');
    expect(styles).toContain('.reasoning-slider::-moz-range-track{height:16px;');
  });
  test('mobile composer keeps the same bottom padding when compacted', () => {
    const styles = css();
    expect(styles).toContain('.composer-wrap:not(.composer-compact),.composer-wrap.composer-compact{padding-bottom:calc(var(--mobile-bottom-nav-height) + 10px + env(safe-area-inset-bottom,0px))}');
  });
});
