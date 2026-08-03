import { describe, expect, it, vi } from 'vitest';
import type { PromptWithVariants, Variant } from './types';
import {
  createPromptDraftState,
  describeDirtyDrafts,
  isVariantDraftDirty,
  promptDraftReducer,
  savePromptDrafts,
} from './promptDrafts';

function variant(id: string, label: string, content: string): Variant {
  return {
    id,
    prompt_id: 'prompt-1',
    label,
    content,
    content_type: 'static',
    variables: null,
    sort_order: 0,
    created_at: null,
    updated_at: null,
  };
}

function prompt(overrides: Partial<PromptWithVariants> = {}): PromptWithVariants {
  return {
    id: 'prompt-1',
    title: 'Original title',
    description: 'Original description',
    primary_variant_id: 'variant-a',
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    variants: [
      variant('variant-a', 'Concise', 'A content'),
      variant('variant-b', 'Detailed', 'B content'),
    ],
    tags: [],
    ...overrides,
  };
}

describe('prompt draft reducer', () => {
  it('persists every dirty variant to its own variant ID', async () => {
    const source = prompt();
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-a',
      content: 'A edited',
    });
    state = promptDraftReducer(state, {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-b',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-b',
      label: 'Detailed edited',
    });

    const updatePrompt = vi.fn().mockResolvedValue(undefined);
    const updateVariant = vi.fn().mockResolvedValue(undefined);
    const result = await savePromptDrafts(state, { updatePrompt, updateVariant });

    expect(updatePrompt).not.toHaveBeenCalled();
    expect(updateVariant.mock.calls).toEqual([
      ['variant-a', 'A edited', 'Concise'],
      ['variant-b', 'B content', 'Detailed edited'],
    ]);
    expect(result.savedVariantIds).toEqual(['variant-a', 'variant-b']);
    expect(result.failures).toEqual([]);
  });

  it('persists metadata once even when it was edited under another variant', async () => {
    const source = prompt();
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_metadata',
      title: 'Edited under A',
    });
    state = promptDraftReducer(state, {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-b',
    });

    const updatePrompt = vi.fn().mockResolvedValue(undefined);
    const updateVariant = vi.fn().mockResolvedValue(undefined);
    const result = await savePromptDrafts(state, { updatePrompt, updateVariant });

    expect(updatePrompt).toHaveBeenCalledTimes(1);
    expect(updatePrompt).toHaveBeenCalledWith('prompt-1', {
      title: 'Edited under A',
      description: 'Original description',
    });
    expect(updateVariant).not.toHaveBeenCalled();
    expect(result.savedMetadata).toBe(true);
    expect(result.failures).toEqual([]);
  });

  it('persists a trimmed-empty description as an explicit null clear', async () => {
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: prompt(),
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_metadata',
      description: '   ',
    });
    const updatePrompt = vi.fn().mockResolvedValue(undefined);

    await savePromptDrafts(state, {
      updatePrompt,
      updateVariant: vi.fn().mockResolvedValue(undefined),
    });

    expect(updatePrompt).toHaveBeenCalledWith('prompt-1', {
      title: 'Original title',
      description: null,
    });
  });

  it('leaves drafts untouched when the same prompt is refetched during editing', () => {
    const source = prompt();
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_metadata',
      title: 'Unsaved title',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-a',
      content: 'Unsaved A content',
    });
    const beforeRefetch = state;
    const refetched = prompt({
      title: 'Server title',
      variants: [
        variant('variant-a', 'Server label', 'Server A content'),
        variant('variant-b', 'Detailed', 'Server B content'),
      ],
    });

    state = promptDraftReducer(state, {
      type: 'seed',
      prompt: refetched,
      activeVariantId: 'variant-a',
    });

    expect(state).toBe(beforeRefetch);
    expect(state.metadata?.title).toBe('Unsaved title');
    expect(state.variants.get('variant-a')?.content).toBe('Unsaved A content');
  });

  it('marks successful saves clean while retaining failed variant drafts', async () => {
    const source = prompt();
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-a',
      content: 'A edited',
    });
    state = promptDraftReducer(state, {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-b',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-b',
      content: 'B edited',
    });

    const result = await savePromptDrafts(state, {
      updatePrompt: vi.fn().mockResolvedValue(undefined),
      updateVariant: vi.fn((variantId: string) =>
        variantId === 'variant-a'
          ? Promise.reject(new Error('A failed'))
          : Promise.resolve(),
      ),
    });
    state = promptDraftReducer(state, {
      type: 'mark_saved',
      metadata: result.savedMetadata,
      variantIds: result.savedVariantIds,
    });

    expect(result.savedVariantIds).toEqual(['variant-b']);
    expect(result.failures).toMatchObject([
      { kind: 'variant', variantId: 'variant-a', label: 'Concise' },
    ]);
    expect(isVariantDraftDirty(state.variants.get('variant-a')!)).toBe(true);
    expect(isVariantDraftDirty(state.variants.get('variant-b')!)).toBe(false);
  });

  it('enumerates dirty metadata and variant labels for exit confirmation', () => {
    const source = prompt();
    let state = promptDraftReducer(createPromptDraftState(), {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-a',
    });
    state = promptDraftReducer(state, {
      type: 'edit_metadata',
      title: 'Unsaved title',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-a',
      content: 'Unsaved A',
    });
    state = promptDraftReducer(state, {
      type: 'seed',
      prompt: source,
      activeVariantId: 'variant-b',
    });
    state = promptDraftReducer(state, {
      type: 'edit_variant',
      variantId: 'variant-b',
      label: 'Detailed renamed',
    });

    expect(describeDirtyDrafts(state)).toBe(
      'Title, and edits to variants: Concise, Detailed renamed',
    );
  });
});
