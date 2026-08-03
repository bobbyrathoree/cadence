import { useState, useEffect, useCallback, useRef } from 'react';
import { api } from '../../lib/api';
import {
  canApplyCopyEffect,
  startCopy,
  type CopyOperation,
} from '../../lib/copy';
import { getPrimaryVariant } from '../../lib/prompt';
import { interpolate, parse } from '../../lib/variables';
import type { PromptListItem, PromptWithVariants } from '../../lib/types';
import { SearchResults } from './SearchResults';
import { SearchPreview } from './SearchPreview';

interface Props {
  revision: number;
  shownRevision: number;
  onFillActiveChange?: (active: boolean) => void;
}

interface InlineFill {
  operation: CopyOperation;
  content: string;
  values: Map<string, string>;
}

export function FloatingSearch({
  revision,
  shownRevision,
  onFillActiveChange = () => undefined,
}: Props) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<PromptListItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [selectedPrompt, setSelectedPrompt] = useState<PromptWithVariants | null>(null);
  const [loading, setLoading] = useState(false);
  const [copyBusy, setCopyBusy] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [fill, setFill] = useState<InlineFill | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const copyRef = useRef<CopyOperation | null>(null);
  const liveRef = useRef(true);

  useEffect(() => {
    liveRef.current = true;
    return () => {
      liveRef.current = false;
      copyRef.current?.fill?.cancel();
      copyRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (shownRevision > 0) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [shownRevision]);

  // Debounced search or load recents
  useEffect(() => {
    let cancelled = false;

    if (query.trim().length === 0) {
      // Show recent prompts when no query
      setLoading(true);
      api.prompts
        .list()
        .then((items) => {
          if (!cancelled) {
            setResults(items);
            setSelectedIndex(0);
          }
        })
        .catch(() => {
          if (!cancelled) setResults([]);
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
      return () => {
        cancelled = true;
      };
    }

    setLoading(true);
    const timer = setTimeout(() => {
      api
        .search(query)
        .then((items) => {
          if (!cancelled) {
            setResults(items);
            setSelectedIndex(0);
          }
        })
        .catch(() => {
          if (!cancelled) setResults([]);
        })
        .finally(() => {
          if (!cancelled) setLoading(false);
        });
    }, 300);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query, revision]);

  // Load preview for selected result
  useEffect(() => {
    let cancelled = false;
    const item = results[selectedIndex];

    if (!item) {
      setSelectedPrompt(null);
      return;
    }

    api.prompts
      .get(item.id)
      .then((prompt) => {
        if (!cancelled) setSelectedPrompt(prompt);
      })
      .catch(() => {
        if (!cancelled) setSelectedPrompt(null);
      });

    return () => {
      cancelled = true;
    };
  }, [results, selectedIndex, revision]);

  const handleResultSelect = useCallback((index: number) => {
    setSelectedIndex(index);
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const promptId = selectedPrompt?.id ?? null;
    const current = copyRef.current;
    if (current && current.key.promptId !== promptId) {
      current.fill?.cancel();
      copyRef.current = null;
      setCopyBusy(false);
      setFill(null);
      setCopyError(null);
      onFillActiveChange(false);
    }
  }, [onFillActiveChange, selectedPrompt?.id]);

  const cancelFill = useCallback(() => {
    fill?.operation.fill?.cancel();
    setFill(null);
    onFillActiveChange(false);
    inputRef.current?.focus();
  }, [fill, onFillActiveChange]);

  const beginCopy = useCallback(
    (prompt: PromptWithVariants) => {
      if (copyBusy) return;
      const variant = getPrimaryVariant(prompt);
      if (!variant) return;
      setCopyError(null);
      const operation = startCopy({
        content: variant.content,
        promptId: prompt.id,
        variantId: variant.id,
        onAccounting: (ok, key) => {
          if (!canApplyCopyEffect(copyRef.current, key, liveRef.current)) return;
          if (!ok) console.warn('Cadence copied the prompt but could not record usage');
        },
      });
      copyRef.current = operation;
      setCopyBusy(true);
      if (operation.fill) {
        setFill({
          operation,
          content: variant.content,
          values: new Map(),
        });
        onFillActiveChange(true);
      }

      void operation.settled.then((result) => {
        if (!canApplyCopyEffect(copyRef.current, operation.key, liveRef.current)) {
          return;
        }
        setCopyBusy(false);
        setFill(null);
        onFillActiveChange(false);
        if (result.kind === 'copied') {
          void import('@tauri-apps/api/core').then(({ invoke }) => {
            invoke('hide_search_window').catch(console.error);
          });
        } else if (result.kind === 'clipboard_error') {
          setCopyError(result.message);
          inputRef.current?.focus();
        }
      });
    },
    [copyBusy, onFillActiveChange],
  );

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (fill) {
        if (e.key === 'Escape') {
          e.preventDefault();
          e.stopPropagation();
          cancelFill();
        }
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && results[selectedIndex]) {
        e.preventDefault();
        const item = results[selectedIndex];
        if (selectedPrompt?.id === item.id) {
          beginCopy(selectedPrompt);
        } else {
          api.prompts.get(item.id).then(beginCopy).catch((error) => {
            setCopyError(String(error).slice(0, 200));
          });
        }
      }
    },
    [beginCopy, cancelFill, fill, results, selectedIndex, selectedPrompt],
  );

  return (
    <div
      className="flex flex-col h-full"
      onKeyDown={handleKeyDown}
      tabIndex={-1}
    >
      {/* Search bar */}
      <div
        className="flex-shrink-0"
        style={{
          padding: '14px 16px',
          borderBottom: '1px solid var(--border)',
        }}
      >
        <div className="flex items-center gap-3">
          {/* Magnifying glass icon */}
          <svg
            width="18"
            height="18"
            viewBox="0 0 24 24"
            fill="none"
            stroke="var(--text-secondary)"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className="flex-shrink-0"
          >
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>

          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search prompts..."
            autoFocus
            className="flex-1 outline-none"
            style={{
              background: 'transparent',
              border: 'none',
              fontSize: '14px',
              color: 'var(--text-primary)',
              fontFamily: 'inherit',
            }}
          />

          <kbd
            style={{
              fontSize: '10px',
              fontWeight: 500,
              padding: '2px 6px',
              borderRadius: 4,
              background: 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
              color: 'var(--text-secondary)',
              fontFamily: 'inherit',
            }}
          >
            ESC
          </kbd>
        </div>
        {copyError && (
          <div role="alert" style={{ marginTop: 6, paddingLeft: 30, color: '#ff453a', fontSize: 11 }}>
            {copyError}
          </div>
        )}
      </div>

      {/* Split view: results + preview */}
      <div className="flex flex-1 min-h-0">
        <SearchResults
          results={results}
          selectedIndex={selectedIndex}
          onSelect={handleResultSelect}
          loading={loading}
        />
        {fill?.operation.fill ? (
          <InlineVariableFill
            fill={fill}
            onValuesChange={(values) => {
              setFill((current) => current ? { ...current, values } : null);
            }}
            onConfirm={() => {
              fill.operation.fill?.resume(fill.values);
              setFill(null);
              onFillActiveChange(false);
            }}
            onCancel={cancelFill}
          />
        ) : (
          <SearchPreview prompt={selectedPrompt} />
        )}
      </div>

      {/* Footer with keyboard shortcuts */}
      <div
        className="flex items-center gap-4 flex-shrink-0"
        style={{
          padding: '8px 16px',
          borderTop: '1px solid var(--border)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
        }}
      >
        <span className="flex items-center gap-1">
          <kbd style={kbdStyle}>&#8593;&#8595;</kbd> Navigate
        </span>
        <span className="flex items-center gap-1">
          <kbd style={kbdStyle}>&#9166;</kbd>{' '}
          {copyBusy ? 'Copying' : 'Copy & close'}
        </span>
        <span className="flex items-center gap-1">
          <kbd style={kbdStyle}>esc</kbd> Dismiss
        </span>
      </div>
    </div>
  );
}

function InlineVariableFill({
  fill,
  onValuesChange,
  onConfirm,
  onCancel,
}: {
  fill: InlineFill;
  onValuesChange: (values: Map<string, string>) => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const preview = interpolate(parse(fill.content), fill.values);
  const names = fill.operation.fill?.names ?? [];
  return (
    <form
      aria-label="Fill prompt variables"
      className="flex-1 min-w-0 overflow-y-auto"
      style={{ padding: '14px 16px' }}
      onSubmit={(event) => {
        event.preventDefault();
        onConfirm();
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.preventDefault();
          event.stopPropagation();
          onCancel();
          return;
        }
        if (event.key !== 'Enter') return;
        const inputs = [...event.currentTarget.querySelectorAll('input')];
        const index = inputs.indexOf(event.target as HTMLInputElement);
        if (index >= 0 && index < inputs.length - 1) {
          event.preventDefault();
          inputs[index + 1]?.focus();
        } else if (index === inputs.length - 1) {
          event.preventDefault();
          onConfirm();
        }
      }}
    >
      <div style={{ marginBottom: 10, fontSize: 12, fontWeight: 600, color: 'var(--text-primary)' }}>
        Fill variables
      </div>
      <div style={{ display: 'grid', gap: 9 }}>
        {names.map((name, index) => (
          <label
            key={name}
            style={{ display: 'grid', gap: 4, fontSize: 10, color: 'var(--text-secondary)' }}
          >
            {name}
            <input
              autoFocus={index === 0}
              aria-label={name}
              value={fill.values.get(name) ?? ''}
              onChange={(event) => {
                const values = new Map(fill.values);
                values.set(name, event.target.value);
                onValuesChange(values);
              }}
              style={{
                height: 30,
                padding: '0 8px',
                border: '1px solid var(--border)',
                borderRadius: 6,
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                fontSize: 11,
              }}
            />
          </label>
        ))}
      </div>
      <pre
        aria-label="Interpolated preview"
        style={{
          margin: '12px 0 0',
          padding: 10,
          border: '1px solid var(--border)',
          borderRadius: 6,
          whiteSpace: 'pre-wrap',
          overflowWrap: 'anywhere',
          color: 'var(--text-primary)',
          fontSize: 10,
          lineHeight: 1.5,
        }}
      >
        {preview}
      </pre>
      <button type="submit" style={{ position: 'absolute', width: 1, height: 1, opacity: 0 }}>
        Copy
      </button>
    </form>
  );
}

const kbdStyle: React.CSSProperties = {
  fontSize: '10px',
  fontWeight: 500,
  padding: '1px 5px',
  borderRadius: 3,
  background: 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
  fontFamily: 'inherit',
};
