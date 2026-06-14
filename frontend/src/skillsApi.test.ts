import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const skillsBackend = () => ['models.rs', 'skills.rs']
  .map((file) => readFileSync(new URL(`../../src/backend/${file}`, import.meta.url), 'utf8'))
  .join('\n');

describe('skills API routing', () => {
  test('yahu skills restore local skill inventory with Hermes Python config helpers', () => {
    const source = skillsBackend();
    expect(source).toContain('state.hermes_home.join("skills")');
    expect(source).toContain('optional-skills');
    expect(source).toContain('collect_skill_dirs(&root, &root, &disabled, &mut found)');
    expect(source).toContain('std::fs::read_to_string(&skill_md)');
    expect(source).toContain('from hermes_cli.skills_config import get_disabled_skills');
    expect(source).toContain('save_disabled_skills(config, disabled)');
    expect(source).toContain('HERMES_WEBUI_PYTHON');
    expect(source).toContain('hermes_python_command(&agent_dir)');
    expect(source).toContain('agent_dir.join("venv/bin/python3")');
    expect(source).toContain('Command::new(python)');
  });

  test('skill file and mutation routes are backed by the restored skill workspace implementation', () => {
    const source = skillsBackend();
    expect(source).toContain('async fn skill_files(');
    expect(source).toContain('async fn skill_file(');
    expect(source).toContain('async fn skill_file_write(');
    expect(source).toContain('async fn skill_toggle(');
    expect(source).toContain('async fn skill_item_rename(');
    expect(source).toContain('async fn skill_item_delete(');
    expect(source).toContain('async fn skill_delete(');
    expect(source).toContain('skill_item_destination_path');
    expect(source).toContain('delete_skill_dir');
    expect(source).toContain('resolve_skill_file_path');
    expect(source).not.toContain('skill_api_unavailable(');
  });
});
