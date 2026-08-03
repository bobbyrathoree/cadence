import { getPrimaryVariant } from '../../lib/prompt';
import type { PromptWithVariants } from '../../lib/types';
import { VariableHighlighter } from '../shared/VariableHighlighter';

interface Props {
  prompt: PromptWithVariants | null;
}

export function SearchPreview({ prompt }: Props) {
  const variant = prompt ? getPrimaryVariant(prompt) ?? null : null;

  if (!prompt) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{
          fontSize: '12px',
          color: 'var(--text-secondary)',
        }}
      >
        Select a prompt to preview
      </div>
    );
  }

  const charCount = variant?.content.length ?? 0;

  return (
    <div className="flex-1 flex flex-col min-w-0">
      {/* Header */}
      <div
        className="flex items-center gap-2 flex-shrink-0"
        style={{
          padding: '10px 14px',
          borderBottom: '1px solid var(--border)',
        }}
      >
        <span
          className="flex-1 truncate"
          style={{
            fontSize: '13px',
            fontWeight: 600,
            color: 'var(--text-primary)',
          }}
        >
          {prompt.title}
        </span>
        {prompt.variants.length > 1 && (
          <span
            style={{
              fontSize: '10px',
              fontWeight: 500,
              padding: '2px 6px',
              borderRadius: 8,
              background: 'color-mix(in srgb, var(--accent) 12%, transparent)',
              color: 'var(--accent)',
            }}
          >
            {prompt.variants.length} variants
          </span>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto" style={{ padding: '12px 14px' }}>
        <pre
          style={{
            fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
            fontSize: '11px',
            lineHeight: 1.6,
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
            color: 'var(--text-primary)',
            margin: 0,
          }}
        >
          {variant && <VariableHighlighter content={variant.content} />}
        </pre>
      </div>

      {/* Footer: tags + char count */}
      <div
        className="flex items-center gap-2 flex-wrap flex-shrink-0"
        style={{
          padding: '8px 14px',
          borderTop: '1px solid var(--border)',
          fontSize: '10px',
          color: 'var(--text-secondary)',
        }}
      >
        {prompt.tags.map((tag) => (
          <span
            key={tag.id}
            className="inline-flex items-center rounded-full"
            style={{
              padding: '1px 6px',
              background: tag.color
                ? `color-mix(in srgb, ${tag.color} 15%, transparent)`
                : 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
              color: tag.color ?? 'var(--text-secondary)',
              fontWeight: 500,
            }}
          >
            {tag.name}
          </span>
        ))}
        <span className="ml-auto" style={{ whiteSpace: 'nowrap' }}>
          {charCount.toLocaleString()} chars
        </span>
      </div>
    </div>
  );
}
