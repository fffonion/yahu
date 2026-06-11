import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const routes = () => readFileSync(new URL('./hashRoute.ts', import.meta.url), 'utf8');
const server = () => ['mod.rs', 'skills.rs']
  .map((file) => readFileSync(new URL(`../../src/backend/${file}`, import.meta.url), 'utf8'))
  .join('\n');

describe('skills UI', () => {
  test('skills mode is available from desktop rail and mobile bottom nav', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'images' | 'workspace' | 'skills' | 'settings'");
    expect(source).toContain('rail-btn nav-skills');
    expect(source).toContain("setNavMode('skills')");
    expect(source).toContain("aria-label={t('nav.skills')}><Star");
    expect(styles).toContain('.rail-btn.nav-skills.active{--rail-accent:#f59e0b}');
    expect(styles).toContain('.rail-btn.nav-chat.active,.rail-btn.nav-cron.active,.rail-btn.nav-memory.active,.rail-btn.nav-skills.active,.rail-btn.nav-images.active');
    expect(styles).toContain('@media (max-width: 760px)');
    expect(styles).not.toContain('.skills-main{display:none!important}');
  });

  test('skills mode lists skills and toggles enablement without auto-opening a skill by default', () => {
    const source = app();
    expect(source).toContain('SkillsSidebar');
    expect(source).toContain('SkillMain');
    expect(source).toContain('SkillWorkspaceAside');
    expect(source).toContain("fetch('/skills/list'");
    expect(source).toContain("fetch(`/skills/toggle/${encodeURIComponent(skill.name)}`");
    expect(source).not.toContain('|| list[0]');
    expect(source).not.toContain('Skill loaded:');
    expect(source).toContain('className="skill-enable-toggle"');
  });

  test('skills are opened only from explicit selection or skill hash routes', () => {
    const source = app();
    expect(source).toContain("const [skillRouteTarget, setSkillRouteTarget] = useState(initialRoute.mode === 'skills' ? initialRoute.skillName || '' : '')");
    expect(source).toContain("if (route.mode === 'skills' && route.skillName) setSkillRouteTarget(route.skillName)");
    expect(source).toContain("if (route.mode === 'skills' && !route.skillName) clearSelectedSkill()");
    expect(source).toContain("writeHashRoute({ mode: 'skills', skillName: skill.name })");
    expect(source).toContain("openSkillFile(skill.name, 'SKILL.md')");
  });

  test('mobile skills drawer closes after selecting a skill row', () => {
    const source = app();
    expect(source).toContain('closeMobileSidebar={closeMobileSidebar} />');
    expect(source).toContain('closeMobileSidebar: () => void');
    expect(source).toContain('onClick={() => { selectSkill(skill); closeMobileSidebar(); }}');
  });

  test('skills mode has a right-side skill file workspace', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("mode === 'skills' && <>");
    expect(source).toContain('className="skill-workspace workspace"');
    expect(source).toContain("fetch(`/skills/files?name=${encodeURIComponent(skill.name)}");
    expect(source).toContain("fetch(`/skills/file?name=${encodeURIComponent(skillName)}");
    expect(styles).toContain('.app-shell.skills-mode{grid-template-columns:360px minmax(480px,1fr) 320px}');
    expect(styles).toContain('.skill-workspace');
  });

  test('hash routes support skills mode', () => {
    const source = routes();
    expect(source).toContain("mode === 'skills'");
    expect(source).toContain("return { mode: 'skills' }");
    expect(source).toContain("return { mode: 'skills', skillName: decodePart(kind) }");
    expect(source).toContain("return route.skillName ? `#/skills/${encodePart(route.skillName)}` : '#/skills'");
  });
});

describe('skills backend API', () => {
  test('serves skill list, skill files, and skill enable toggles from root-confined routes', () => {
    const source = server();
    expect(source).toContain('.route("/skills/list", get(skills_list))');
    expect(source).toContain('.route("/skills/files", get(skill_files))');
    expect(source).toContain('.route("/skills/file", get(skill_file))');
    expect(source).toContain('.route("/skills/toggle/{name}", post(skill_toggle))');
    expect(source).toContain('find_skill_dir');
    expect(source).toContain('resolve_skill_file_path');
    expect(source).toContain('skills_config import get_disabled_skills, save_disabled_skills');
  });
});
