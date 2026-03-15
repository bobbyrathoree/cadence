import type { Variant } from '../../lib/types';

interface Props {
  variants: Variant[];
  selectedId: string;
  onSelect: (id: string) => void;
}

export function VariantSelector({ variants, selectedId, onSelect }: Props) {
  // 1 variant: render nothing
  if (variants.length <= 1) return null;

  // 2 variants: segmented control
  if (variants.length === 2) {
    return (
      <div
        className="flex-shrink-0"
        style={{
          padding: '10px 16px',
          borderBottom: '1px solid var(--border)',
        }}
      >
        <div
          className="inline-flex rounded-md overflow-hidden"
          style={{
            background: 'color-mix(in srgb, var(--text-secondary) 10%, transparent)',
          }}
        >
          {variants.map((v) => {
            const isActive = v.id === selectedId;
            return (
              <button
                key={v.id}
                onClick={() => onSelect(v.id)}
                className="cursor-default outline-none"
                style={{
                  padding: '5px 14px',
                  fontSize: '11px',
                  fontWeight: 500,
                  border: 'none',
                  borderRadius: 5,
                  margin: 2,
                  background: isActive ? 'var(--accent)' : 'transparent',
                  color: isActive ? '#ffffff' : 'var(--text-secondary)',
                  transition: 'all 0.15s ease',
                }}
              >
                {v.label}
              </button>
            );
          })}
        </div>
      </div>
    );
  }

  // 3+ variants: horizontal tabs with underline
  return (
    <div
      className="flex gap-0 flex-shrink-0 overflow-x-auto"
      style={{
        borderBottom: '1px solid var(--border)',
        padding: '0 16px',
      }}
    >
      {variants.map((v) => {
        const isActive = v.id === selectedId;
        return (
          <button
            key={v.id}
            onClick={() => onSelect(v.id)}
            className="cursor-default outline-none flex-shrink-0"
            style={{
              padding: '10px 14px',
              fontSize: '11px',
              fontWeight: 500,
              border: 'none',
              borderBottom: isActive
                ? '2px solid var(--accent)'
                : '2px solid transparent',
              background: 'transparent',
              color: isActive ? 'var(--accent)' : 'var(--text-secondary)',
              transition: 'all 0.15s ease',
            }}
          >
            {v.label}
          </button>
        );
      })}
    </div>
  );
}
