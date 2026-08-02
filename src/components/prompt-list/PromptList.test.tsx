import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PromptListItem } from '../../lib/types';
import { PromptList } from './PromptList';

const mocks = vi.hoisted(() => ({
  setDisplayedPromptIds: vi.fn(),
  scrollIntoView: vi.fn(),
  searchQuery: 'query',
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({
    searchQuery: mocks.searchQuery,
    setSearchQuery: vi.fn(),
    selectedPromptId: 'search-2',
    setSelectedPromptId: vi.fn(),
    setIsCreating: vi.fn(),
    requestEditExit: vi.fn(),
    setDisplayedPromptIds: mocks.setDisplayedPromptIds,
  }),
}));

vi.mock('../../lib/hooks', () => ({
  useSearch: () => ({
    data: [item('search-1'), item('search-2')],
    error: null,
    loading: false,
  }),
}));

function item(id: string): PromptListItem {
  return {
    id,
    title: id,
    description: null,
    snippet: 'Snippet',
    snippet_runs: [],
    is_favorite: false,
    variant_count: 1,
    copy_count: 0,
    last_copied_at: null,
    tags: [],
  };
}

describe('PromptList displayed navigation source', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Element.prototype.scrollIntoView = mocks.scrollIntoView;
  });

  afterEach(cleanup);

  it('publishes search-result IDs and scrolls the selected displayed row', async () => {
    mocks.searchQuery = 'query';
    render(
      <PromptList
        prompts={[item('backing-page')]}
        promptsLoading={false}
        promptsError={null}
        onRetry={vi.fn()}
        hasMore={false}
        loadingMore={false}
        onLoadMore={vi.fn()}
      />,
    );

    await waitFor(() =>
      expect(mocks.setDisplayedPromptIds).toHaveBeenCalledWith([
        'search-1',
        'search-2',
      ]),
    );
    expect(mocks.scrollIntoView).toHaveBeenCalledWith({ block: 'nearest' });
  });

  it('renders a retryable error instead of an empty state', () => {
    mocks.searchQuery = '';
    const onRetry = vi.fn();
    render(
      <PromptList
        prompts={[]}
        promptsLoading={false}
        promptsError={new Error('offline')}
        onRetry={onRetry}
        hasMore={false}
        loadingMore={false}
        onLoadMore={vi.fn()}
      />,
    );

    expect(screen.getByRole('alert')).toHaveTextContent("Couldn't load prompts");
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});
