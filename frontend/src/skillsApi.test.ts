import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const skillsBackend = () => readFileSync(new URL('../../src/backend/skills.rs', import.meta.url), 'utf8');

describe('skills API routing', () => {
  test('yahu skills list proxies Hermes API Server and does not scan HERMES_HOME skill files', () => {
    const source = skillsBackend();
    expect(source).toContain('format!("{}/v1/skills", state.api_url)');
    expect(source).toContain('state.client.request(reqwest::Method::GET, url)');
    expect(source).toContain('response_from_reqwest(state, None, resp).await');
    expect(source).not.toContain('state.hermes_home.join("skills")');
    expect(source).not.toContain('optional-skills');
    expect(source).not.toContain('SKILL.md');
    expect(source).not.toContain('std::fs::read_to_string');
    expect(source).not.toContain('fs::read_dir');
    expect(source).not.toContain('fs::remove_dir_all');
    expect(source).not.toContain('fs::rename');
    expect(source).not.toContain('save_disabled_skills');
  });

  test('skill file and mutation routes fail closed until Hermes API Server exposes them', () => {
    const source = skillsBackend();
    expect(source).toContain('skill_api_unavailable("skill file listing")');
    expect(source).toContain('skill_api_unavailable("skill file reading")');
    expect(source).toContain('skill_api_unavailable("skill toggling")');
    expect(source).toContain('skill_api_unavailable("skill item rename")');
    expect(source).toContain('skill_api_unavailable("skill item delete")');
    expect(source).toContain('skill_api_unavailable("skill deletion")');
    expect(source).toContain('StatusCode::NOT_IMPLEMENTED');
  });
});
