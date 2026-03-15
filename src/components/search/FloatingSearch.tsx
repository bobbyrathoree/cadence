import { useState, useEffect, useCallback } from 'react';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { api } from '../../lib/api';
import type { PromptListItem, PromptWithVariants } from '../../lib/types';
import { SearchResults } from './SearchResults';
import { SearchPreview } from './SearchPreview';

export function FloatingSearch() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<PromptListItem[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [selectedPrompt, setSelectedPrompt] = useState<PromptWithVariants | null>(null);
  const [loading, setLoading] = useState(false);

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
      return () => { cancelled = true; };
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
  }, [query]);

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

    return () => { cancelled = true; };
  }, [results, selectedIndex]);

  // Keyboard navigation
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' && results[selectedIndex]) {
        e.preventDefault();
        const item = results[selectedIndex];
        // Copy to clipboard and hide
        api.prompts.get(item.id).then((prompt) => {
          const variant = prompt.variants[0];
          if (variant) {
            writeText(variant.content).catch(console.error);
            api.prompts.recordCopy(prompt.id, variant.id).catch(console.error);
          }
          import('@tauri-apps/api/core').then(({ invoke }) => {
            invoke('hide_search_window').catch(console.error);
          });
        });
      }
    },
    [results, selectedIndex],
  );

  return (
    <div className="flex flex-col h-full" onKeyDown={handleKeyDown}>
      {/* Search bar */}
      <div
        className="flex items-center gap-3 flex-shrink-0"
        style={{
          padding: '14px 16px',
          borderBottom: '1px solid var(--border)',
        }}
      >
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

        {/* ESC badge */}
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

      {/* Split view: results + preview */}
      <div className="flex flex-1 min-h-0">
        <SearchResults
          results={results}
          selectedIndex={selectedIndex}
          onSelect={setSelectedIndex}
          loading={loading}
        />
        <SearchPreview prompt={selectedPrompt} />
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
          <kbd style={kbdStyle}>&#9166;</kbd> Copy &amp; close
        </span>
        <span className="flex items-center gap-1">
          <kbd style={kbdStyle}>esc</kbd> Dismiss
        </span>
      </div>
    </div>
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
