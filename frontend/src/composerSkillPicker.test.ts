import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const backend = () => readFileSync(new URL('../../src/backend/skills.rs', import.meta.url), 'utf8');
const routes = () => readFileSync(new URL('../../src/backend/mod.rs', import.meta.url), 'utf8');

describe('composer skill picker and popover language', () => {
  test('plus opens upload and recent skill picker above the composer input', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('className="composer-skill-picker"');
    expect(source).toContain("fetch('/skills/recent'");
    expect(source).toContain('setTimeout(() => {');
    expect(source).toContain('}, 180);');
    expect(source).toContain('chooseComposerSkill');
    expect(source).toContain('composer-skill-chip');
    expect(styles).toContain('.composer-skill-picker{position:absolute;left:0;right:0;bottom:calc(100% + 10px)');
    expect(styles).toContain('grid-template-columns:17px max-content minmax(0,1fr)');
    expect(styles).toContain('text-overflow:ellipsis;white-space:nowrap;color:var(--muted)');
  });

  test('details popover uses compact nested menu styling without a speech-bubble arrow', () => {
    const styles = css();
    expect(styles).toContain('.composer-details-popover{position:absolute;right:0;bottom:calc(100% + 10px)');
    expect(styles).toContain('.composer-submenu-row{');
    expect(styles).toContain('.composer-model-list{');

  });

  test('Rust backend records recent skills independently of browser storage', () => {
    expect(routes()).toContain('.route("/skills/recent", get(skills_recent).post(skill_recent_record))');
    expect(backend()).toContain('skill-recent.json');
    expect(backend()).toContain('fs::write(recent_skills_path(&state), bytes)');
  });

  test('stop action uses a filled square while the send action remains unchanged', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('primaryActionIsStop ? <Square fill="currentColor" /> : <ArrowUp />');
    expect(styles).toContain('.composer-footer .composer-primary-btn.is-stop svg{width:14px;height:14px;stroke-width:2.4;fill:currentColor}');
  });

  test('selected skill stays inline with prompt text while typing', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('const composerSkillText = composerSkillVisible ? props.input.slice(composerSkillPrefix.length).replace(/^\\s+/, \'\') : props.input;');
    expect(source).toContain('props.setInput(value ? `${composerSkillPrefix} ${value}` : composerSkillPrefix);');
    expect(source).toContain('className="composer-skill-textarea"');
    expect(source).toContain('value={composerSkillVisible ? composerSkillText : props.input}');
    expect(styles).toContain('.composer-input-wrap{position:relative;display:flex;align-items:flex-start;min-width:0}');
    expect(styles).toContain('.composer-input-wrap>.composer-skill-chip{margin-top:15px}');
    expect(styles).toContain('.composer-skill-chip{position:static;display:inline-flex;');
    expect(styles).toContain('color:#10b981;');
  });

  test('navigation active colors stay fixed and distinct across themes', () => {
    const styles = css();
    for (const [name, color] of Object.entries({
      chat: '#3b82f6', cron: '#f59e0b', memory: '#8b5cf6', skills: '#10b981', insights: '#06b6d4', images: '#ec4899', workspace: '#84cc16', terminal: '#ef4444', settings: '#64748b',
    })) expect(styles).toContain(`.rail-btn.nav-${name}.active{--rail-accent:${color}!important}`);
    expect(styles).toContain('color:var(--rail-accent)!important;background:color-mix(in srgb,var(--rail-accent) 22%,transparent)!important');
    expect(styles).toContain('.mobile-bottom-nav .rail-btn.nav-chat.active');
  });

  test('left rail colors stay distinct across themes', () => {
    const styles = css();
    for (const [name, color] of Object.entries({
      chat: '#3b82f6', cron: '#f59e0b', memory: '#8b5cf6', skills: '#10b981', insights: '#06b6d4', images: '#ec4899', workspace: '#84cc16', terminal: '#ef4444', settings: '#64748b',
    })) expect(styles).toContain(`.rail-btn.nav-${name}.active{--rail-accent:${color}!important}`);
  });
});
