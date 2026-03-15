import type { PromptListItem as PromptListItemType } from '../../lib/types';

interface Props {
  item: PromptListItemType;
  isSelected: boolean;
  onClick: () => void;
}

export function PromptListItem({ item, isSelected, onClick }: Props) {
  return (
    <button
      onClick={onClick}
      className="w-full text-left flex items-start gap-2 cursor-default"
      style={{
        padding: '10px 14px',
        borderBottom: '1px solid var(--border)',
        borderLeft: isSelected ? '3px solid var(--accent)' : '3px solid transparent',
        background: isSelected
          ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
          : 'transparent',
        transition: 'background 0.1s ease',
      }}
      onMouseEnter={(e) => {
        if (!isSelected) {
          e.currentTarget.style.background =
            'color-mix(in srgb, var(--text-secondary) 8%, transparent)';
        }
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = isSelected
          ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
          : 'transparent';
      }}
    >
      <div className="flex-1 min-w-0">
        <div
          className="font-semibold truncate"
          style={{ fontSize: '13px', color: 'var(--text-primary)' }}
        >
          {item.title}
        </div>
        <div
          className="truncate mt-0.5"
          style={{
            fontSize: '11px',
            color: 'var(--text-secondary)',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {item.snippet}
        </div>
        {item.variant_count > 1 && (
          <span
            className="inline-block mt-1"
            style={{ fontSize: '10px', color: 'var(--text-secondary)' }}
          >
            {item.variant_count} variants
          </span>
        )}
      </div>
      {item.is_favorite && (
        <span
          className="flex-shrink-0 mt-0.5"
          style={{ fontSize: '11px', color: '#f5a623' }}
          aria-label="Favorite"
        >
          <svg
            width="12"
            height="12"
            viewBox="0 0 16 16"
            fill="#f5a623"
            stroke="none"
          >
            <path d="M8 1.5l1.85 3.75 4.15.6-3 2.93.71 4.12L8 10.88 4.29 12.9 5 8.78 2 5.85l4.15-.6z" />
          </svg>
        </span>
      )}
    </button>
  );
}
