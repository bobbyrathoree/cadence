import type React from 'react';

interface CollectionItemProps {
  icon?: React.ReactNode;
  name: string;
  count?: number;
  isActive?: boolean;
  onClick: () => void;
  color?: string;
}

export function CollectionItem({
  icon,
  name,
  count,
  isActive = false,
  onClick,
}: CollectionItemProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center w-full gap-2 px-2.5 py-1.5 rounded-md text-left transition-colors duration-100 cursor-default"
      style={{
        fontSize: '12.5px',
        background: isActive ? 'color-mix(in srgb, var(--accent) 15%, transparent)' : 'transparent',
        color: isActive ? 'var(--accent)' : 'var(--text-primary)',
      }}
      onMouseEnter={(e) => {
        if (!isActive) {
          e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 10%, transparent)';
        }
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = isActive
          ? 'color-mix(in srgb, var(--accent) 15%, transparent)'
          : 'transparent';
      }}
    >
      {icon && <span className="flex-shrink-0 w-4 flex items-center justify-center">{icon}</span>}
      <span className="flex-1 truncate">{name}</span>
      {count !== undefined && (
        <span
          className="flex-shrink-0 tabular-nums"
          style={{ fontSize: '11px', color: 'var(--text-secondary)' }}
        >
          {count}
        </span>
      )}
    </button>
  );
}
