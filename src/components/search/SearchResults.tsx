import { useEffect, useRef } from 'react';
import type { PromptListItem } from '../../lib/types';

interface Props {
  results: PromptListItem[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  loading: boolean;
}

export function SearchResults({ results, selectedIndex, onSelect, loading }: Props) {
  const listRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Map<number, HTMLDivElement>>(new Map());

  // Scroll selected item into view
  useEffect(() => {
    const el = itemRefs.current.get(selectedIndex);
    if (el) {
      el.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedIndex]);

  if (loading && results.length === 0) {
    return (
      <div
        className="flex items-center justify-center"
        style={{
          width: '35%',
          borderRight: '1px solid var(--border)',
          fontSize: '12px',
          color: 'var(--text-secondary)',
        }}
      >
        Loading...
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div
        className="flex items-center justify-center"
        style={{
          width: '35%',
          borderRight: '1px solid var(--border)',
          fontSize: '12px',
          color: 'var(--text-secondary)',
        }}
      >
        No results found
      </div>
    );
  }

  return (
    <div
      ref={listRef}
      className="overflow-y-auto"
      style={{
        width: '35%',
        borderRight: '1px solid var(--border)',
      }}
    >
      {results.map((item, index) => {
        const isSelected = index === selectedIndex;
        return (
          <div
            key={item.id}
            ref={(el) => {
              if (el) itemRefs.current.set(index, el);
              else itemRefs.current.delete(index);
            }}
            onClick={() => onSelect(index)}
            className="cursor-default"
            style={{
              padding: '10px 12px',
              borderLeft: isSelected ? '3px solid var(--accent)' : '3px solid transparent',
              background: isSelected
                ? 'color-mix(in srgb, var(--accent) 8%, transparent)'
                : 'transparent',
              transition: 'background 0.1s ease',
            }}
          >
            <div className="flex items-center gap-2">
              {/* Title */}
              <span
                className="flex-1 truncate"
                style={{
                  fontSize: '13px',
                  fontWeight: isSelected ? 600 : 400,
                  color: 'var(--text-primary)',
                }}
              >
                {item.title}
              </span>

              {/* Favorite star */}
              {item.is_favorite && (
                <span style={{ fontSize: '12px', color: '#ffcc00', flexShrink: 0 }}>
                  &#9733;
                </span>
              )}

              {/* Variant count */}
              {item.variant_count > 1 && (
                <span
                  style={{
                    fontSize: '10px',
                    fontWeight: 500,
                    padding: '1px 5px',
                    borderRadius: 8,
                    background: 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
                    color: 'var(--text-secondary)',
                    flexShrink: 0,
                  }}
                >
                  {item.variant_count}v
                </span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
