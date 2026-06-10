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

  test('selecting a model updates local session state and carries provider without patching session metadata', () => {
    const source = app();
    expect(source).toContain('const changeSessionModel = useCallback((nextModel: string, option?: ModelOption) =>');
    expect(source).toContain('const resolvedModel = realModelOrEmpty(nextModel)');
    expect(source).toContain('setSelectedModelProvider(provider)');
    expect(source).toContain('modelRef.current = resolvedModel');
    expect(source).toContain('providerRef.current = provider');
    expect(source).toContain('{ ...old, model: resolvedModel, provider }');
    expect(source).toContain('{ ...s, model: resolvedModel, provider }');
    expect(source).toContain('const sessionBody = sessionProvider ? { model: sessionModel, provider: sessionProvider } : { model: sessionModel };');
    expect(source).toContain('const sessionModel = realModelOrEmpty(modelRef.current) || createdSession?.model || activeSession?.model || activeSessionDetail?.model ||');
    expect(source).toContain('const sessionProvider = providerRef.current || createdSession?.provider || activeSession?.provider || activeSessionDetail?.provider ||');
    expect(source).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
    expect(source).toContain('setSelectedModelProvider((current) => activeProvider !== current ? activeProvider : current)');
    expect(source).toContain('}, [activeSession?.model, activeSession?.provider]);');
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
    expect(source).toContain('valueProvider={sessionProvider}');
    expect(source).toContain('key={modelOptionKey(item)}');
    expect(source).not.toContain('props.models.filter((m: ModelOption) => m.id !== currentModel)');
  });

  test('bottom model and reasoning controls are icon-only with searchable capped menus', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('ariaLabel="Model"');
    expect(source).toContain('hideLabel');
    expect(source).toContain('searchable');
    expect(source).not.toContain('label="Model"');
    expect(source).not.toContain('label="Reasoning"');
    expect(source).toContain('className="dropdown-search"');
    expect(source).toContain("${searchable ? 'searchable' : ''}");
    expect(styles).toContain('.dropdown-menu{position:absolute;bottom:calc(100% + 8px);');
    expect(styles).toContain('max-height:320px;overflow:auto');
    expect(styles).toContain('.dropdown-control.searchable .dropdown-menu{width:min(640px,calc(100vw - 48px));max-height:min(420px,calc(100vh - 180px));padding:10px;gap:6px;grid-template-rows:auto}');
    expect(styles).toContain('.dropdown-control.searchable .dropdown-search{height:42px;margin:0 0 8px;padding:0 13px;border-radius:12px;background:var(--bg);box-shadow:none}');
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
});
