import { expect, test } from 'bun:test';
import { readFileSync } from 'fs';

const app = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

test('installed counters exclude the optional library and the views are separate', () => {
  expect(app).toContain('const installedSkills = skills.filter((skill) => skill.installed !== false);');
  expect(app).toContain('const enabledCount = installedSkills.filter((skill) => skill.enabled !== false).length;');
  expect(app).toContain("t('skills.optionalLibrary')");
  expect(app).toContain("role=\"tablist\"");
  expect(app).toContain('body.optional || []');
});

test('optional skills have no enable switch or editing action', () => {
  expect(app).toContain('skill.installed !== false && <span className="skill-enable-toggle"');
  expect(app).toContain('if (skill.mutable === false) return;');
  expect(app).toContain('readOnly={skill?.mutable === false}');
  expect(app).toContain("!readOnly && !editMode && preview.kind === 'text'");
});
