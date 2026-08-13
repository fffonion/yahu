import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';

const source = () => readFileSync(new URL('./ChatTranscript.tsx', import.meta.url), 'utf8');

describe('streaming turn detail auto-follow', () => {
  test('scrolls an opened streaming detail to its latest content', () => {
    const transcript = source();
    expect(transcript).toContain('const detailGroupRef = useRef<HTMLDetailsElement>(null);');
    expect(transcript).toContain("if (scroller) scroller.scrollTop = scroller.scrollHeight;");

    expect(transcript).toContain('if (!open || (!item.defaultOpen && !forceOpenToken) || !detailMessages.length || !followLatestRef.current) return;');
    expect(transcript).toContain('ref={detailGroupRef}');
    expect(transcript).toContain('open={open || forceOpenToken > 0}');
    expect(transcript).toContain('if (!open) return;\n    loadDetails();');
    expect(transcript).toContain('!streaming && forceOpenLatestDetailToken > 0');
    expect(transcript).toContain('const followLatestRef = useRef(!!item.defaultOpen || forceOpenToken > 0);');
    expect(transcript).toContain('followLatestRef.current = distanceFromBottom <= 32;');
    expect(transcript).toContain('|| !followLatestRef.current) return;');
    expect(transcript).toContain("scroller.addEventListener('scroll', onScroll, { passive: true });");
    expect(transcript).toContain("scroller.style.overflowAnchor = 'none';");
  });
});
