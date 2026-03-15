import React, { useState, useEffect, useMemo } from 'react';
import { usePromptDetail } from '../../lib/hooks';
import { VariantSelector } from './VariantSelector';
import { TagPills } from './TagPills';
import { CopyButton } from '../shared/CopyButton';

interface Props {
  promptId: string;
}

/**
 * Highlight template variables in prompt content.
 * Mustache-style vars get blue highlights, bracket placeholders get orange.
 */
function highlightVariables(content: string): React.ReactNode[] {
  const pattern = /(\{\{[^}]+\}\}|\[[A-Z][A-Z _]*\])/g;
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      parts.push(content.slice(lastIndex, match.index));
    }

    const token = match[0];
    const isMustache = token.startsWith('{{');

    parts.push(
      <span
        key={`${match.index}-${token}`}
        className="rounded px-1"
        style={{
          background: isMustache
            ? 'color-mix(in srgb, #007aff 18%, transparent)'
            : 'color-mix(in srgb, #ff9500 18%, transparent)',
          color: isMustache ? '#4dabff' : '#ffb84d',
          fontWeight: 500,
        }}
      >
        {token}
      </span>,
    );

    lastIndex = match.index + token.length;
  }

  if (lastIndex < content.length) {
    parts.push(content.slice(lastIndex));
  }

  return parts;
}

function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return 'Never';
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);

  if (diffMin < 1) return 'Just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDays = Math.floor(diffHr / 24);
  if (diffDays < 30) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

function formatDate(dateStr: string | null): string {
  if (!dateStr) return '--';
  return new Date(dateStr).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function PromptDetail({ promptId }: Props) {
  const { prompt, loading } = usePromptDetail(promptId);
  const [selectedVariantId, setSelectedVariantId] = useState<string | null>(null);

  // Reset selected variant when prompt changes
  useEffect(() => {
    if (prompt) {
      setSelectedVariantId(prompt.primary_variant_id ?? prompt.variants[0]?.id ?? null);
    }
  }, [prompt]);

  const selectedVariant = useMemo(() => {
    if (!prompt || !selectedVariantId) return null;
    return prompt.variants.find((v) => v.id === selectedVariantId) ?? prompt.variants[0] ?? null;
  }, [prompt, selectedVariantId]);

  const highlightedContent = useMemo(() => {
    if (!selectedVariant) return [];
    return highlightVariables(selectedVariant.content);
  }, [selectedVariant]);

  if (loading) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: '13px' }}
      >
        Loading...
      </div>
    );
  }

  if (!prompt) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: '13px' }}
      >
        Prompt not found
      </div>
    );
  }

  const charCount = selectedVariant?.content.length ?? 0;

  return (
    <div className="flex flex-col h-full">
      {/* Sticky header */}
      <div
        className="flex-shrink-0"
        style={{
          padding: '16px 20px 12px',
          borderBottom: '1px solid var(--border)',
          background: 'var(--bg-secondary)',
        }}
      >
        {/* Title row */}
        <div className="flex items-center gap-3">
          <h2
            className="flex-1 min-w-0 truncate"
            style={{
              fontSize: '16px',
              fontWeight: 600,
              color: 'var(--text-primary)',
              margin: 0,
            }}
          >
            {prompt.title}
          </h2>
          {selectedVariant && (
            <CopyButton
              content={selectedVariant.content}
              promptId={prompt.id}
              variantId={selectedVariant.id}
            />
          )}
          <button
            className="flex items-center justify-center rounded cursor-default"
            style={{
              padding: '6px 12px',
              fontSize: '12px',
              fontWeight: 500,
              border: '1px solid var(--border)',
              background: 'transparent',
              color: 'var(--text-secondary)',
              borderRadius: 6,
            }}
          >
            Edit
          </button>
        </div>

        {/* Tags and copy stats */}
        <div className="flex items-center gap-3 mt-2 flex-wrap">
          <TagPills tags={prompt.tags} promptId={prompt.id} />
          <span
            style={{
              fontSize: '11px',
              color: 'var(--text-secondary)',
              whiteSpace: 'nowrap',
            }}
          >
            Copied {prompt.copy_count} time{prompt.copy_count !== 1 ? 's' : ''}
            {prompt.last_copied_at && (
              <> &middot; Last: {formatRelativeTime(prompt.last_copied_at)}</>
            )}
          </span>
        </div>
      </div>

      {/* Variant selector */}
      <VariantSelector
        variants={prompt.variants}
        selectedId={selectedVariantId ?? ''}
        onSelect={setSelectedVariantId}
      />

      {/* Content area */}
      <div className="flex-1 overflow-y-auto" style={{ padding: '20px' }}>
        <pre
          style={{
            fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
            fontSize: '12px',
            lineHeight: 1.7,
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
            color: 'var(--text-primary)',
            margin: 0,
          }}
        >
          {highlightedContent}
        </pre>
      </div>

      {/* Footer */}
      <div
        className="flex items-center gap-4 flex-shrink-0"
        style={{
          padding: '10px 20px',
          borderTop: '1px solid var(--border)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
        }}
      >
        <span>Created {formatDate(prompt.created_at)}</span>
        <span>Modified {formatDate(prompt.updated_at)}</span>
        <span>{charCount.toLocaleString()} characters</span>
      </div>
    </div>
  );
}
