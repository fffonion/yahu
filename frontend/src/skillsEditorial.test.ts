import { expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { skillDisplayName, skillDisplayDescription, skillMatchesQuery } from './skillDisplay';

const source = readFileSync(new URL('./App.tsx', import.meta.url), 'utf8');

test('skills views display editorial copy while routes retain canonical names', () => {
  expect(source).toContain('editorial_name?: string');
  expect(source).toContain('editorial_description?: string');
  expect(source).toContain('skillDisplayName(skill)');
  expect(source).toContain('skillDisplayDescription(skill)');
  expect(source).toContain('skillMatchesQuery(s, query)');
  expect(source).toContain('name=${encodeURIComponent(skill.name)}');
  expect(source).toContain("writeHashRoute({ mode: 'skills', skillName: skill.name })");
});

test('editorial display copy leaves canonical routing metadata intact', () => {
  const skill = { name: 'canonical-skill', description: 'Model instructions', editorial_name: ' Display title ', editorial_description: ' Human-facing copy ' };
  expect(skillDisplayName(skill)).toBe('Display title');
  expect(skillDisplayDescription(skill)).toBe('Human-facing copy');
  expect(skill.name).toBe('canonical-skill');
  expect(skill.description).toBe('Model instructions');
});

test('display copy falls back independently for missing or blank editorial fields', () => {
  const skill = { name: 'canonical-skill', description: 'Canonical copy', editorial_name: ' ', editorial_description: '' };
  expect(skillDisplayName(skill)).toBe('canonical-skill');
  expect(skillDisplayDescription(skill)).toBe('Canonical copy');
  expect(skillDisplayName(null)).toBe('');
  expect(skillDisplayDescription(null)).toBe('');
});

test('search matches canonical and editorial names and descriptions', () => {
  const skill = { name: 'canonical-skill', description: 'Model instructions', editorial_name: 'Display title', editorial_description: 'Human-facing copy' };
  for (const query of ['CANONICAL', 'model', 'display', 'human-facing']) expect(skillMatchesQuery(skill, query)).toBe(true);
  expect(skillMatchesQuery(skill, 'unmatched')).toBe(false);
});
