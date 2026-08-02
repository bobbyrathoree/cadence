import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { usePrompts } from './hooks';

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
});
