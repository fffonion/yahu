type SkillDisplayCopy = {
  name: string;
  description?: string;
  editorial_name?: string;
  editorial_description?: string;
};

export function skillDisplayName(skill?: SkillDisplayCopy | null): string {
  return skill?.editorial_name?.trim() || skill?.name || '';
}

export function skillDisplayDescription(skill?: SkillDisplayCopy | null): string {
  return skill?.editorial_description?.trim() || skill?.description || '';
}

export function skillMatchesQuery(skill: SkillDisplayCopy, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  return [skill.name, skill.description || '', skillDisplayName(skill), skillDisplayDescription(skill)]
    .some((value) => value.toLowerCase().includes(normalized));
}
