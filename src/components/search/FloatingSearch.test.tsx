import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import type { PromptListItem, PromptWithVariants } from '../../lib/types';
import { FloatingSearch } from './FloatingSearch';

const mocks = vi.hoisted(() => ({
  list: vi.fn(),
  get: vi.fn(),
  search: vi.fn(),
  recordCopy: vi.fn(),
  writeText: vi.fn(),
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

const items: PromptListItem[] = [
  {
    id: 'prompt-a',
    title: 'Alpha',
    description: null,
    snippet: 'Alpha content',
    snippet_runs: [],
    is_favorite: false,
    variant_count: 1,
    copy_count: 0,
    last_copied_at: null,
    tags: [],
  },
  {
    id: 'prompt-b',
    title: 'Beta',
    description: null,
    snippet: 'Beta content',
    snippet_runs: [],
    is_favorite: false,
    variant_count: 1,
    copy_count: 0,
    last_copied_at: null,
    tags: [],
  },
];

function prompt(id: string): PromptWithVariants {
  return {
    id,
    title: id === 'prompt-a' ? 'Alpha' : 'Beta',
    description: null,
    primary_variant_id: `${id}-variant`,
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    tags: [],
    variants: [
      {
        id: `${id}-variant`,
        prompt_id: id,
        label: 'Default',
        content: `${id} content`,
        content_type: 'static',
        variables: null,
        sort_order: 0,
        created_at: null,
        updated_at: null,
      },
    ],
  };
}

describe('FloatingSearch', () => {
  beforeAll(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.list.mockResolvedValue(items);
    mocks.get.mockImplementation((id: string) => Promise.resolve(prompt(id)));
    mocks.search.mockResolvedValue(items);
    mocks.recordCopy.mockResolvedValue('');
    mocks.writeText.mockResolvedValue(undefined);
  });

  it('refetches when the owning SearchApp revision changes', async () => {
    const { rerender } = render(
      <FloatingSearch revision={0} shownRevision={0} />,
    );

    await waitFor(() => expect(mocks.list).toHaveBeenCalledTimes(1));
    rerender(<FloatingSearch revision={1} shownRevision={0} />);
    await waitFor(() => expect(mocks.list).toHaveBeenCalledTimes(2));
  });

  it('selects the query on show and restores input focus after result clicks', async () => {
    const { rerender } = render(
      <FloatingSearch revision={0} shownRevision={0} />,
    );
    const input = screen.getByPlaceholderText<HTMLInputElement>('Search prompts...');
    fireEvent.change(input, { target: { value: 'alpha query' } });

    rerender(<FloatingSearch revision={1} shownRevision={1} />);
    expect(input).toHaveFocus();
    expect(input).toHaveValue('alpha query');
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe('alpha query'.length);

    await screen.findByText('Beta');
    input.blur();
    expect(input).not.toHaveFocus();
    fireEvent.click(screen.getByText('Beta'));
    expect(input).toHaveFocus();
  });
});
