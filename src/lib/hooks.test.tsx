import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { usePrompts } from './hooks';
import type { PromptListItem } from './types';

const mocks = vi.hoisted(() => ({
  list: vi.fn(),
}));

vi.mock('./api', () => ({
  api: {
    prompts: { list: mocks.list },
  },
}));

describe('fetch hook contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('keeps errors distinct from empty data and clears them on retry', async () => {
    mocks.list.mockRejectedValueOnce(new Error('database unavailable'));
    const { result, rerender } = renderHook(
      ({ revision }) => usePrompts(revision),
      { initialProps: { revision: 0 } },
    );

    await waitFor(() =>
      expect(result.current.error?.message).toBe('database unavailable'),
    );
    expect(result.current.data).toEqual([]);
    expect(result.current.loading).toBe(false);

    mocks.list.mockResolvedValueOnce([]);
    rerender({ revision: 1 });

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.data).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('loads 100-row pages and deduplicates IDs without changing the next offset', async () => {
    const first = Array.from({ length: 100 }, (_, index) => item(`prompt-${index}`));
    const second = [
      item('prompt-99'),
      ...Array.from({ length: 99 }, (_, index) => item(`prompt-${index + 100}`)),
    ];
    mocks.list
      .mockResolvedValueOnce(first)
      .mockResolvedValueOnce(second);
    const { result } = renderHook(() => usePrompts(0, 'favorites'));

    await waitFor(() => expect(result.current.hasMore).toBe(true));
    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.loadingMore).toBe(false));

    expect(result.current.data).toHaveLength(199);
    expect(new Set(result.current.data.map((prompt) => prompt.id))).toHaveProperty(
      'size',
      199,
    );
    expect(mocks.list).toHaveBeenNthCalledWith(1, 'favorites', 100, 0);
    expect(mocks.list).toHaveBeenNthCalledWith(2, 'favorites', 100, 100);
  });
});

function item(id: string): PromptListItem {
  return {
    id,
    title: id,
    description: null,
    snippet: '',
    snippet_runs: [],
    is_favorite: false,
    variant_count: 1,
    copy_count: 0,
    last_copied_at: null,
    tags: [],
  };
}
