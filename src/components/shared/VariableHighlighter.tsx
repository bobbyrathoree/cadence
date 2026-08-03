import { useMemo } from 'react';
import { parse } from '../../lib/variables';

export function VariableHighlighter({ content }: { content: string }) {
  const segments = useMemo(() => parse(content), [content]);
  return (
    <>
      {segments.map((segment, index) => {
        if (segment.kind === 'text') return segment.raw;
        const variable = segment.kind === 'variable';
        return (
          <span
            key={`${index}-${segment.raw}`}
            className="rounded px-1"
            style={{
              background: variable
                ? 'color-mix(in srgb, #007aff 18%, transparent)'
                : 'color-mix(in srgb, #ff9500 18%, transparent)',
              color: variable ? '#4dabff' : '#ffb84d',
              fontWeight: 500,
            }}
          >
            {segment.raw}
          </span>
        );
      })}
    </>
  );
}
