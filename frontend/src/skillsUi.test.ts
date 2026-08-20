import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = () => readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');
const i18n = () => readFileSync(new URL('./i18n.ts', import.meta.url), 'utf8');
const css = () => readFileSync(new URL('./styles.css', import.meta.url), 'utf8');
const routes = () => readFileSync(new URL('./hashRoute.ts', import.meta.url), 'utf8');
const server = () => ['mod.rs', 'models.rs', 'skills.rs', 'tests.rs', 'tests/core.rs']
  .map((file) => readFileSync(new URL(`../../src/backend/${file}`, import.meta.url), 'utf8'))
  .join('\n');

describe('skills UI', () => {
  test('skills mode is available from desktop rail and mobile bottom nav', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("type Mode = 'chat' | 'cron' | 'memory' | 'insights' | 'images' | 'workspace' | 'skills' | 'terminal' | 'settings'");
    expect(source).toContain('rail-btn nav-skills');
    expect(source).toContain("setNavMode('skills')");
    expect(source).toContain("aria-label={t('nav.skills')}><Puzzle");
    expect(styles).toContain('.rail-btn.nav-skills.active{--rail-accent:#f59e0b}');
    expect(styles).toContain('.rail-btn.nav-chat.active,.rail-btn.nav-cron.active,.rail-btn.nav-memory.active,.rail-btn.nav-skills.active,.rail-btn.nav-images.active');

    expect(styles).toContain('@media (max-width: 760px)');

  });

  test('skills mode lists skills and toggles enablement without auto-opening a skill by default', () => {
    const source = app();
    expect(source).toContain('SkillsSidebar');
    expect(source).toContain('SkillMain');
    expect(source).toContain('SkillWorkspaceAside');
    expect(source).toContain("fetch('/skills/list'");
    expect(source).toContain("fetch(`/skills/toggle/${encodeURIComponent(skill.name)}`");

    expect(source).toContain('className="skill-enable-toggle"');
  });

  test('skills header shows enabled and installed totals', () => {
    const source = app();
    const translations = i18n();
    expect(source).toContain('const enabledCount = skills.filter((skill) => skill.enabled !== false).length;');
    expect(source).toContain("{enabledCount} {t('skills.enabledCount')} | {skills.length} {t('skills.installed')}");
    expect(translations).toContain("'skills.enabledCount': { en: 'enabled', 'zh-CN': '已启用', 'zh-TW': '已啟用', ja: '有効' }");
    expect(translations).toContain("'skills.installed': { en: 'installed', 'zh-CN': '已安装', 'zh-TW': '已安裝', ja: 'インストール済み' }");
  });

  test('skill rows expose a right-click delete menu with confirmation', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('type SkillContextMenu = { skill: Skill; x: number; y: number } | null;');
    expect(source).toContain('const [skillMenu, setSkillMenu] = useState<SkillContextMenu>(null);');
    expect(source).toContain('openSkillMenu(skill, ev)');
    expect(source).toContain('className="skill-context-menu"');
    expect(source).toContain('deleteSkill(skillMenu.skill)');
    expect(source).toContain("fetch(`/skills/${encodeURIComponent(skill.name)}`, { method: 'DELETE' })");
    expect(source).toContain("requestConfirm(t('skills.deleteTitle'), tf('skills.deleteConfirm', skill.name), true)");
    expect(styles).toContain('.skill-context-menu');
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
    expect(source).toContain("saveUrl={skill ? (path: string) => `/skills/file?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(path)}` : undefined}");
    expect(source).toContain('const target = saveUrl ? saveUrl(preview.path) : `/workspace/file?path=${encodeURIComponent(preview.path)}`;');
    expect(styles).toContain('.app-shell.skills-mode{grid-template-columns:var(--sidebar-width) minmax(480px,1fr) 320px}');
    expect(styles).toContain('.skill-workspace');
  });

  test('skill workspace header has a download skill zip button', () => {
    const source = app();
    const skillsWorkspace = source.slice(source.indexOf('function SkillWorkspaceAside'), source.indexOf('function WorkspaceEditorPreview'));
    expect(skillsWorkspace).toContain('skill');
    expect(skillsWorkspace).toContain('Download');
    expect(skillsWorkspace).toContain('/skills/download/');
    expect(skillsWorkspace).toContain('triggerSkillDownload');
    expect(skillsWorkspace).toContain('aria-label');
  });

  test('skill markdown preview shrinks before the right file workspace instead of sliding underneath it', () => {
    const styles = css();
    expect(styles).toContain('.workspace-editor-preview{width:100%;max-width:100%;min-width:0;');
    expect(styles).toContain('.workspace-text-preview{width:100%;max-width:100%;min-width:0;');
    expect(styles).toContain('.workspace-markdown-preview{width:100%;max-width:100%;min-width:0;');
  });

  test('long skill subtitles use a clipped one-way seamless marquee without exposing a horizontal scrollbar', () => {
    const source = app();
    const styles = css();
    expect(source).toContain('function MarqueeText');
    expect(source).toContain('const itemRef = useRef<HTMLSpanElement>(null)');
    expect(source).toContain('item.scrollWidth > node.clientWidth');
    expect(source).toContain('new ResizeObserver');
    expect(source).toContain('skill-subtitle-marquee ${scrolling');
    expect(source).toContain('className="skill-subtitle-item" aria-hidden="true"');
    expect(styles).toContain('.skill-subtitle-marquee{');
    expect(styles).toContain('overflow:hidden');
    expect(styles).toContain('animation:skill-subtitle-scroll 18s linear 1s infinite');
    expect(styles).toContain('to{transform:translateX(calc(-1 * var(--skill-subtitle-cycle)))}');

  });

  test('skill version history reuses the shared dropdown in the preview toolbar after edit and close', () => {
    const source = app();
    const styles = css();
    const preview = source.slice(source.indexOf('function WorkspaceEditorPreview'), source.indexOf('function WorkspaceBrowser'));

    expect(source).toContain('toolbarExtra={skill ? <DropdownControl');
    expect(source).toContain('placement="down"');
    expect(source).toContain('iconOnly');
    expect(preview).toContain('{toolbarExtra}</div></div>');
    expect(preview.indexOf("aria-label={t('workspace.closePreview')}")).toBeLessThan(preview.indexOf('{toolbarExtra}'));
    expect(styles).toContain('.skill-history-dropdown{position:relative;min-width:34px;width:34px;flex:0 0 34px;');
  });

  test('shared dropdown closes on outside pointer interaction and supports toolbar placement', () => {
    const source = app();
    expect(source).toContain("placement = 'up'");
    expect(source).toContain("emptyLabel = t('chat.noModels')");
    expect(source).toContain("placement === 'down' ? 'drop-down' : 'drop-up'");
    expect(source).toContain("window.addEventListener('pointerdown', closeOutside)");
    expect(source).toContain('if (rootRef.current?.contains(event.target as Node)) return;');
    expect(source).toContain("emptyLabel={t('skills.noVersions')}");
  });

  test('version history options use the selected skill version from each snapshot', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("fetch(`/skills/backups?name=${encodeURIComponent(skill.name)}`");
    expect(source).toContain('description?: string');
    expect(source).toContain('className="dropdown-option-copy"');
    expect(source).toContain('className="dropdown-option-label"');
    expect(source).toContain('className="dropdown-option-description"');
    expect(source).toContain('label: String(backup.version)');

    expect(source).toContain("description: String(backup.reason || t('skills.snapshot'))");
    expect(styles).toContain('.skill-history-dropdown .dropdown-option-label{font-family:ui-monospace');
    expect(styles).toContain('.skill-history-dropdown .dropdown-option-description{');
  });

  test('skill file workspace rows expose right-click rename and delete actions', () => {
    const source = app();
    expect(source).toContain('type SkillFileContextMenu = { skill: Skill; entry: WorkspaceEntry; x: number; y: number } | null;');
    expect(source).toContain('const [skillFileMenu, setSkillFileMenu] = useState<SkillFileContextMenu>(null);');
    expect(source).toContain('openSkillFileMenu(skill, entry, ev)');
    expect(source).toContain('className="skill-file-context-menu"');
    expect(source).toContain('renameSkillFileEntry(skillFileMenu.skill, skillFileMenu.entry)');
    expect(source).toContain('deleteSkillFileEntry(skillFileMenu.skill, skillFileMenu.entry)');
    expect(source).toContain("fetch(`/skills/item?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(entry.path)}`, { method: 'PATCH'");
    expect(source).toContain("fetch(`/skills/item?name=${encodeURIComponent(skill.name)}&path=${encodeURIComponent(entry.path)}`, { method: 'DELETE' })");
    expect(source).toContain("requestConfirm(t('skills.deleteFileTitle'), tf('skills.deleteFileConfirm', entry.kind, entry.name), true)");
  });

  test('hash routes support skills mode', () => {
    const source = routes();
    expect(source).toContain("mode === 'skills'");
    expect(source).toContain("return { mode: 'skills' }");
    expect(source).toContain("return { mode: 'skills', skillName: decodePart(kind) }");
    expect(source).toContain("return route.skillName ? `#/skills/${encodePart(route.skillName)}` : '#/skills'");
  });
  test('skill list uses the API Server and supports wheel and pan scrolling', () => {
    const source = app();
    const styles = css();
    expect(source).toContain("fetch('/skills/list', { cache: 'no-store' })");
    expect(source).not.toContain('readSkillListCache');
    expect(source).not.toContain('SKILL_LIST_CACHE_KEY');
    expect(source).toContain("useEffect(() => { if (mode === 'skills') loadSkills(); }, [mode, loadSkills]);");
    expect(source).toContain("ref={skillsListRef} className=\"skills-list sessions\"");
    expect(source).toContain('onWheel={handleSkillWheel}');
    expect(source).toContain('onPointerDown={handleSkillPointerDown}');
    expect(source).toContain('onPointerMove={handleSkillPointerMove}');
    expect(source).toContain('onPointerUp={finishSkillPan}');
    expect(source).toContain('onWheel={(event) => event.stopPropagation()}');
    expect(styles).toContain('.skills-list{min-height:0;overflow:auto;touch-action:pan-y;overscroll-behavior:contain;cursor:grab}');
  });

  test('skill category controls remain clickable while list panning is enabled', () => {
    const source = app();
    const skillsSidebar = source.slice(source.indexOf('function SkillsSidebar'), source.indexOf('type SkillBackup'));
    expect(skillsSidebar).toContain('const handleSkillPointerDown = (event: React.PointerEvent<HTMLDivElement>) =>');
    expect(skillsSidebar).toContain("event.target.closest('button, input, [role=\"switch\"], .section-label')");
    expect(skillsSidebar).toContain('<div className="section-label" role="button" tabIndex={0} aria-expanded={expandedCats.has(cat)}');
    expect(skillsSidebar).not.toContain('<button type="button" className="section-label"');
  });
});


describe('skills backend API', () => {
  test('routes skills through restored local/Python-backed skill implementation', () => {
    const source = server();
    expect(source).toContain('.route("/skills/list", get(skills_list))');
    expect(server()).toContain('/v1/skills');
    expect(server()).toContain('fetch_api_json');
    expect(source).toContain('.route("/skills/files", get(skill_files))');
    expect(source).toContain('.route("/skills/file", get(skill_file).put(skill_file_write))');
    expect(source).toContain('.route("/skills/download/{name}", get(skill_download))');
    expect(source).toContain('.route("/skills/toggle/{name}", post(skill_toggle))');
    expect(source).toContain('"/skills/item"');
    expect(source).toContain('patch(skill_item_rename).delete(skill_item_delete)');
    expect(source).toContain('.route("/skills/{name}", delete(skill_delete))');
    expect(source).toContain('state.hermes_home.join("skills")');
    expect(source).toContain('HERMES_WEBUI_PYTHON');
    expect(source).toContain('skill_item_destination_path');
    expect(source).toContain('delete_skill_dir');
    expect(source).toContain('find_skill_dir');
    expect(source).toContain('skill_path_is_archived');
    expect(source).toContain('skills_collector_skips_archive_directories');
    expect(source).toContain('resolve_skill_file_path');
    expect(source).toContain('skills_config import get_disabled_skills, save_disabled_skills');

  });
});
