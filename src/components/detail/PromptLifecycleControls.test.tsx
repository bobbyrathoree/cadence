import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PromptWithVariants } from '../../lib/types';
import {
  formatPromptUsage,
  PromptLifecycleControls,
} from './PromptLifecycleControls';

const mocks = vi.hoisted(() => ({
  usage: vi.fn(),
  deletePrompt: vi.fn(),
  addVariant: vi.fn(),
  setSelectedPromptId: vi.fn(),
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({
    activeCollectionId: null,
    refreshCounter: 0,
    setSelectedPromptId: mocks.setSelectedPromptId,
  }),
}));

vi.mock('../../lib/hooks', () => ({
  useCollections: () => ({ collections: [], loading: false }),
}));

vi.mock('../../lib/api', () => ({
  api: {
    prompts: {
      usage: mocks.usage,
      delete: mocks.deletePrompt,
    },
    variants: {
      add: mocks.addVariant,
      delete: vi.fn(),
    },
    tags: {
      addToPrompt: vi.fn(),
      removeFromPrompt: vi.fn(),
    },
    collections: {
      addPrompt: vi.fn(),
      removePrompt: vi.fn(),
    },
  },
}));

function prompt(): PromptWithVariants {
  return {
    id: 'prompt-1',
    title: 'Prompt one',
    description: null,
    primary_variant_id: 'variant-1',
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    tags: [],
    variants: [
      {
        id: 'variant-1',
        prompt_id: 'prompt-1',
        label: 'Default',
        content: 'Content',
        content_type: 'static',
        variables: null,
        sort_order: 0,
        created_at: null,
        updated_at: null,
      },
    ],
  };
}

describe('PromptLifecycleControls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.usage.mockResolvedValue({
      playbook_count: 2,
      step_count: 3,
      playbook_titles: ['Launch', 'Review'],
    });
    mocks.deletePrompt.mockResolvedValue(undefined);
    mocks.addVariant.mockResolvedValue({
      ...prompt().variants[0],
      id: 'variant-2',
      label: 'Concise',
    });
  });

  afterEach(cleanup);

  it('renders playbook usage before deleting a prompt', async () => {
    render(
      <PromptLifecycleControls
        prompt={prompt()}
        selectedVariantId="variant-1"
        isEditing={false}
        hasUnsavedDrafts={false}
        onSelectVariant={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Delete prompt' }));
    expect(
      await screen.findByText(
        /Used in 3 step\(s\) across 2 playbook\(s\): Launch, Review\./,
      ),
    ).toBeInTheDocument();
    const dialog = screen.getByRole('dialog', { name: 'Delete "Prompt one"?' });
    fireEvent.click(
      within(dialog).getByRole('button', { name: 'Delete prompt' }),
    );

    await waitFor(() => expect(mocks.deletePrompt).toHaveBeenCalledWith('prompt-1'));
    expect(mocks.setSelectedPromptId).toHaveBeenCalledWith(null);
  });

  it('creates a variant and selects the returned ID', async () => {
    const onSelectVariant = vi.fn();
    render(
      <PromptLifecycleControls
        prompt={prompt()}
        selectedVariantId="variant-1"
        isEditing
        hasUnsavedDrafts={false}
        onSelectVariant={onSelectVariant}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add variant' }));
    fireEvent.change(screen.getByLabelText('New variant label'), {
      target: { value: 'Concise' },
    });
    fireEvent.change(screen.getByLabelText('New variant content'), {
      target: { value: 'Short content' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() =>
      expect(mocks.addVariant).toHaveBeenCalledWith(
        'prompt-1',
        'Concise',
        'Short content',
      ),
    );
    expect(onSelectVariant).toHaveBeenCalledWith('variant-2');
  });
});

describe('formatPromptUsage', () => {
  it('formats zero and populated usage deterministically', () => {
    expect(
      formatPromptUsage({
        playbook_count: 0,
        step_count: 0,
        playbook_titles: [],
      }),
    ).toBe('This prompt is not used in any playbooks.');
    expect(
      formatPromptUsage({
        playbook_count: 1,
        step_count: 2,
        playbook_titles: ['Launch'],
      }),
    ).toBe('Used in 2 step(s) across 1 playbook(s): Launch.');
  });
});
