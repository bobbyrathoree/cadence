import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PromptListItem, PromptWithVariants } from '../../lib/types';
import { FloatingSearch } from './FloatingSearch';

const mocks = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  search: vi.fn(),
  recordCopy: vi.fn(),
  writeText: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: {
    prompts: {
      list: mocks.list,
      get: mocks.get,
      recordCopy: mocks.recordCopy,
    },
    search: mocks.search,
  },
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

const item: PromptListItem = {
  id: 'prompt-a',
  title: 'Alpha',
  description: null,
  snippet: 'Hello {{name}}',
  snippet_runs: [],
  is_favorite: false,
  variant_count: 1,
  copy_count: 0,
  last_copied_at: null,
  tags: [],
};

function prompt(): PromptWithVariants {
  return {
    id: 'prompt-a',
    title: 'Alpha',
    description: null,
    primary_variant_id: 'variant-a',
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    tags: [],
    variants: [
      {
        id: 'variant-a',
        prompt_id: 'prompt-a',
        label: 'Default',
        content: 'Hello {{name}}',
        content_type: 'static',
        variables: null,
        sort_order: 0,
        created_at: null,
        updated_at: null,
      },
    ],
  };
}

describe('FloatingSearch variable copy', () => {
  beforeAll(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.list.mockResolvedValue([item]);
    mocks.get.mockResolvedValue(prompt());
    mocks.search.mockResolvedValue([item]);
    mocks.recordCopy.mockResolvedValue('');
    mocks.writeText.mockResolvedValue(undefined);
    mocks.invoke.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  async function openFill() {
    const input = screen.getByPlaceholderText('Search prompts...');
    await screen.findByText('Alpha');
    fireEvent.keyDown(input, { key: 'Enter' });
    return screen.findByLabelText('name');
  }

  it('fills in-panel and copies on Enter through the last field', async () => {
    const fillActive = vi.fn();
    render(
      <FloatingSearch
        revision={0}
        shownRevision={0}
        onFillActiveChange={fillActive}
      />,
    );
    const field = await openFill();
    expect(fillActive).toHaveBeenCalledWith(true);
    fireEvent.change(field, { target: { value: 'Ada' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(mocks.writeText).toHaveBeenCalledWith('Hello Ada'));
    expect(mocks.recordCopy).toHaveBeenCalledWith('prompt-a', 'variant-a');
    await waitFor(() =>
      expect(mocks.invoke).toHaveBeenCalledWith('hide_search_window'),
    );
  });

  it('returns to results on Escape with no copy side effects', async () => {
    render(<FloatingSearch revision={0} shownRevision={0} />);
    const field = await openFill();
    fireEvent.keyDown(field, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByLabelText('name')).toBeNull());
    expect(mocks.writeText).not.toHaveBeenCalled();
    expect(mocks.recordCopy).not.toHaveBeenCalled();
    expect(mocks.invoke).not.toHaveBeenCalled();
  });

  it('renders clipboard failures inline and leaves the window open', async () => {
    mocks.writeText.mockRejectedValue(new Error('clipboard unavailable'));
    render(<FloatingSearch revision={0} shownRevision={0} />);
    const field = await openFill();
    fireEvent.change(field, { target: { value: 'Ada' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'clipboard unavailable',
    );
    expect(mocks.recordCopy).not.toHaveBeenCalled();
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});
