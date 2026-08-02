import { useEffect, useRef, useState } from 'react';
import { useAppContext } from '../../lib/context';
import { useSearch } from '../../lib/hooks';
import type { PromptListItem as PromptListItemType } from '../../lib/types';
import { PromptListItem } from './PromptListItem';

interface Props {
  prompts: PromptListItemType[];
  promptsLoading: boolean;
  promptsError: Error | null;
  onRetry: () => void;
}

export function PromptList({
  prompts,
  promptsLoading,
  promptsError,
  onRetry,
}: Props) {
  const {
    searchQuery,
    setSearchQuery,
    selectedPromptId,
    setSelectedPromptId,
    setIsCreating,
    requestEditExit,
    setDisplayedPromptIds,
  } = useAppContext();

  const [searchRetryCounter, setSearchRetryCounter] = useState(0);
  const {
    data: searchResults,
    error: searchError,
    loading: searchLoading,
  } = useSearch(searchQuery, searchRetryCounter);

  const isSearching = searchQuery.length >= 2;
  const displayItems = isSearching ? searchResults : prompts;
  const loading = isSearching ? searchLoading : promptsLoading;
  const error = isSearching ? searchError : promptsError;
  const itemRefs = useRef<Map<string, HTMLButtonElement>>(new Map());

  useEffect(() => {
    setDisplayedPromptIds(displayItems.map((item) => item.id));
  }, [displayItems, setDisplayedPromptIds]);

  useEffect(() => {
    if (selectedPromptId) {
      itemRefs.current.get(selectedPromptId)?.scrollIntoView({ block: 'nearest' });
    }
  }, [selectedPromptId]);

  return (
    <div
      className="flex flex-col flex-shrink-0 overflow-hidden"
      style={{
        width: 280,
        borderRight: '1px solid var(--border)',
        background: 'var(--bg-primary)',
      }}
    >
      {/* Search bar */}
      <div
        className="flex items-center gap-2 flex-shrink-0"
        style={{
          padding: '10px 12px',
          borderBottom: '1px solid var(--border)',
        }}
      >
        <span
          className="flex-shrink-0"
          style={{ color: 'var(--text-secondary)' }}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 20 20"
            fill="none"
          >
            <circle
              cx="8.5"
              cy="8.5"
              r="5.75"
              stroke="currentColor"
              strokeWidth="2"
            />
            <line
              x1="13"
              y1="13"
              x2="17"
              y2="17"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            />
          </svg>
        </span>
        <input
          type="text"
          placeholder="Search prompts..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="flex-1 min-w-0 outline-none"
          style={{
            fontSize: '12px',
            background: 'transparent',
            color: 'var(--text-primary)',
            border: 'none',
          }}
        />
        <kbd
          className="flex-shrink-0 flex items-center justify-center select-none"
          style={{
            fontSize: '10px',
            fontFamily: 'inherit',
            color: 'var(--text-secondary)',
            background: 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
            borderRadius: 4,
            padding: '2px 6px',
          }}
        >
          {'\u2318'}F
        </kbd>
      </div>

      {/* Scrollable list */}
      <div className="flex-1 overflow-y-auto">
        {loading && displayItems.length === 0 ? (
          <div
            className="p-4 text-center"
            style={{ fontSize: '12px', color: 'var(--text-secondary)' }}
          >
            Loading...
          </div>
        ) : error ? (
          <div
            role="alert"
            className="p-4 text-center"
            style={{ fontSize: 12, color: 'var(--text-secondary)' }}
          >
            <div>Couldn't load prompts</div>
            <button
              type="button"
              onClick={() => {
                if (isSearching) setSearchRetryCounter((counter) => counter + 1);
                else onRetry();
              }}
              style={{
                marginTop: 8,
                padding: '5px 10px',
                border: '1px solid var(--border)',
                borderRadius: 5,
                background: 'transparent',
                color: 'var(--accent)',
                fontSize: 11,
              }}
            >
              Retry
            </button>
          </div>
        ) : displayItems.length === 0 ? (
          <div
            className="p-4 text-center"
            style={{ fontSize: '12px', color: 'var(--text-secondary)' }}
          >
            {isSearching ? 'No results found' : 'No prompts yet'}
          </div>
        ) : (
          displayItems.map((item) => (
            <PromptListItem
              key={item.id}
              item={item}
              isSelected={selectedPromptId === item.id}
              itemRef={(element) => {
                if (element) itemRefs.current.set(item.id, element);
                else itemRefs.current.delete(item.id);
              }}
              onClick={() => {
                if (item.id !== selectedPromptId) {
                  requestEditExit(() => setSelectedPromptId(item.id));
                }
              }}
            />
          ))
        )}
      </div>

      {/* Bottom bar */}
      <div
        className="flex items-center justify-between flex-shrink-0"
        style={{
          padding: '8px 12px',
          borderTop: '1px solid var(--border)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
        }}
      >
        <span>
          {isSearching
            ? `${searchResults.length} result${searchResults.length !== 1 ? 's' : ''}`
            : `${prompts.length} prompt${prompts.length !== 1 ? 's' : ''}`}
        </span>
        <button
          className="flex items-center justify-center rounded cursor-default"
          style={{
            width: 22,
            height: 22,
            fontSize: '16px',
            color: 'var(--text-secondary)',
            background: 'transparent',
            border: 'none',
            transition: 'background 0.1s ease',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background =
              'color-mix(in srgb, var(--text-secondary) 12%, transparent)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent';
          }}
          title="New Prompt"
          onClick={() => {
            requestEditExit(() => setIsCreating(true));
          }}
        >
          +
        </button>
      </div>
    </div>
  );
}
