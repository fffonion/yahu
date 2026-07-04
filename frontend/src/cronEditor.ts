export type CronJobEditable = { name?: string; schedule?: string | { display?: string; expr?: string }; prompt?: string; script?: string | null; deliver?: string | null };

export function cronEditableValues(job: CronJobEditable) {
  const schedule = typeof job.schedule === 'string' ? job.schedule : (job.schedule?.display || job.schedule?.expr || '');
  return {
    name: job.name || '',
    schedule,
    prompt: job.prompt || '',
    script: job.script || '',
    deliver: job.deliver || '',
  };
}

export function buildCronPatch(values: { name: string; schedule: string; prompt: string; script: string; deliver: string }) {
  return {
    name: values.name,
    schedule: values.schedule,
    prompt: values.prompt,
    script: values.script || null,
    deliver: values.deliver,
  };
}
