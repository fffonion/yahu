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
    expect(source).toContain('{ ...old, model: resolvedModel, provider }');
    expect(source).toContain('{ ...s, model: resolvedModel, provider }');
    expect(source).toContain('buildChatRequestBody(payloadInput, sessionModel, effort, sessionProvider)');
    expect(source).toContain('setModelState((current) => activeModel !== current ? activeModel : current)');
    expect(source).toContain('}, [activeSession?.model]);');
    expect(source).not.toContain('if (activeModel && activeModel !== model) setModelState(activeModel);');
    expect(source).not.toContain('body: JSON.stringify({ model: resolvedModel })');
    expect(source).toContain('setModel={changeSessionModel}');
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
    expect(styles).toContain('.dropdown-menu{position:absolute;bottom:calc(100% + 8px);');
    expect(styles).toContain('max-height:320px;overflow:auto');
    expect(styles).toContain('.dropdown-search');
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
    expect(styles).toContain('.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow:visible;position:relative;z-index:160}');
    expect(styles).toContain('.composer-box,.composer-footer{overflow:visible}');
    expect(styles).toContain('.dropdown-control.open{z-index:170}');
    expect(styles).toContain('.dropdown-control.open .dropdown-menu{z-index:180}');
    expect(styles).not.toContain('.main-panel,.chat-header,.chat-scroll,.composer-wrap{min-width:0;width:100%;max-width:100vw;overflow-x:hidden}');
  });
});
