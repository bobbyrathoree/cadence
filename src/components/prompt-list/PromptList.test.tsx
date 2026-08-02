import { cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PromptListItem } from '../../lib/types';
import { PromptList } from './PromptList';

const mocks = vi.hoisted(() => ({
  setDisplayedPromptIds: vi.fn(),
  scrollIntoView: vi.fn(),
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({
    searchQuery: 'query',
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
    results: [item('search-1'), item('search-2')],
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
    render(<PromptList prompts={[item('backing-page')]} promptsLoading={false} />);

    await waitFor(() =>
      expect(mocks.setDisplayedPromptIds).toHaveBeenCalledWith([
        'search-1',
        'search-2',
      ]),
    );
    expect(mocks.scrollIntoView).toHaveBeenCalledWith({ block: 'nearest' });
  });
});
