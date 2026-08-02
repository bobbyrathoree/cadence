import type { Tag } from '../../lib/types';

interface Props {
  tags: Tag[];
  onRemove?: (tagId: string) => void;
}

export function TagPills({ tags, onRemove }: Props) {
  if (tags.length === 0) return null;

  return (
    <div className="flex flex-wrap gap-1.5">
      {tags.map((tag) => (
        <span
          key={tag.id}
          className="inline-flex items-center rounded-full"
          style={{
            fontSize: '10px',
            fontWeight: 500,
            padding: '2px 8px',
            background: tag.color
              ? `color-mix(in srgb, ${tag.color} 15%, transparent)`
              : 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
            color: tag.color ?? 'var(--text-secondary)',
          }}
        >
          {tag.name}
          {onRemove && (
            <button
              type="button"
              aria-label={`Remove tag ${tag.name}`}
              onClick={() => onRemove(tag.id)}
              style={{
                width: 14,
                height: 14,
                marginLeft: 4,
                padding: 0,
                border: 0,
                background: 'transparent',
                color: 'currentColor',
                fontSize: 13,
                lineHeight: '12px',
              }}
            >
              {'\u00d7'}
            </button>
          )}
        </span>
      ))}
    </div>
  );
}
