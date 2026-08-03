import { useMemo, useState } from 'react';
import { interpolate, parse } from '../../lib/variables';
import { Modal } from './Modal';
import { VariableHighlighter } from './VariableHighlighter';

interface Props {
  id: string;
  content: string;
  names: string[];
  onConfirm: (values: Map<string, string>) => void;
  onCancel: () => void;
}

export function FillVariablesModal({
  id,
  content,
  names,
  onConfirm,
  onCancel,
}: Props) {
  const [values, setValues] = useState<Map<string, string>>(() => new Map());
  const segments = useMemo(() => parse(content), [content]);
  const preview = useMemo(
    () => interpolate(segments, values),
    [segments, values],
  );

  return (
    <Modal
      id={id}
      isOpen
      onClose={onCancel}
      ariaLabel="Fill prompt variables"
      width={520}
      maxHeight="80vh"
      panelStyle={{ padding: 20, overflowY: 'auto' }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          onConfirm(new Map(values));
        }}
      >
        <h3 style={{ margin: 0, fontSize: 15, color: 'var(--text-primary)' }}>
          Fill variables
        </h3>
        <div style={{ marginTop: 14, display: 'grid', gap: 10 }}>
          {names.map((name) => (
            <label
              key={name}
              style={{ display: 'grid', gap: 4, fontSize: 11, color: 'var(--text-secondary)' }}
            >
              {name}
              <input
                aria-label={name}
                autoFocus={name === names[0]}
                value={values.get(name) ?? ''}
                onChange={(event) => {
                  const value = event.target.value;
                  setValues((current) => {
                    const next = new Map(current);
                    next.set(name, value);
                    return next;
                  });
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Escape') {
                    event.preventDefault();
                    event.stopPropagation();
                    onCancel();
                    return;
                  }
                  if (event.key !== 'Enter') return;
                  event.preventDefault();
                  onConfirm(new Map(values));
                }}
                style={{
                  height: 32,
                  padding: '0 9px',
                  border: '1px solid var(--border)',
                  borderRadius: 6,
                  background: 'var(--bg-primary)',
                  color: 'var(--text-primary)',
                  fontSize: 12,
                  outline: 'none',
                }}
              />
            </label>
          ))}
        </div>
        <div
          aria-label="Interpolated preview"
          style={{
            marginTop: 14,
            padding: 12,
            border: '1px solid var(--border)',
            borderRadius: 6,
            whiteSpace: 'pre-wrap',
            overflowWrap: 'anywhere',
            fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
            fontSize: 11,
            lineHeight: 1.6,
            color: 'var(--text-primary)',
          }}
        >
          <VariableHighlighter content={preview} />
        </div>
        <div className="flex justify-end gap-2" style={{ marginTop: 16 }}>
          <button type="button" onClick={onCancel} style={secondaryButtonStyle}>
            Cancel
          </button>
          <button type="submit" style={primaryButtonStyle}>
            Copy
          </button>
        </div>
      </form>
    </Modal>
  );
}

const secondaryButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  border: '1px solid var(--border)',
  borderRadius: 6,
  background: 'transparent',
  color: 'var(--text-secondary)',
  fontSize: 12,
};

const primaryButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  border: 0,
  borderRadius: 6,
  background: 'var(--accent)',
  color: '#fff',
  fontSize: 12,
  fontWeight: 600,
};
