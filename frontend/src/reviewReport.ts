import type { ReviewComment, ReviewState } from './reviewComments';

function compareText(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0;
}

function compareComments(left: ReviewComment, right: ReviewComment): number {
  return left.anchor.line - right.anchor.line
    || compareText(left.anchor.side, right.anchor.side)
    || left.createdAt - right.createdAt;
}

export function buildReviewReport(state: ReviewState, meta: { workspace: string; generatedAt: number }): string {
  const grouped = new Map<string, ReviewComment[]>();
  for (const comment of state.comments) {
    const comments = grouped.get(comment.path) ?? [];
    comments.push(comment);
    grouped.set(comment.path, comments);
  }

  const sections = [...grouped.entries()]
    .sort(([leftPath], [rightPath]) => compareText(leftPath, rightPath))
    .map(([path, comments]) => {
      const lines = comments
        .sort(compareComments)
        .map((comment) => `- line ${comment.anchor.line} (${comment.anchor.side}): ${comment.body.replace(/\r\n|\r|\n/g, ' ')}`);
      return `## ${path}\n\n${lines.join('\n')}`;
    });
  const metadata = `Generated at ${new Date(meta.generatedAt).toISOString()} | Workspace: ${meta.workspace} | Files: ${state.files.length} | Comments: ${state.comments.length}`;
  return [`# Code Review Report`, metadata, ...sections].join('\n\n');
}

export function reviewReportPrompt(absPath: string): string {
  return `用户做了代码审查，结论在${absPath}中，请解决`;
}

function pad2(value: number): string {
  return String(value).padStart(2, '0');
}

export function reviewFileName(now = Date.now()): string {
  const date = new Date(now);
  const year = String(date.getFullYear()).padStart(4, '0');
  return `review-${year}${pad2(date.getMonth() + 1)}${pad2(date.getDate())}-${pad2(date.getHours())}${pad2(date.getMinutes())}${pad2(date.getSeconds())}.md`;
}
