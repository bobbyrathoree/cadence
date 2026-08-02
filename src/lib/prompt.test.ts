import { describe, expect, it } from 'vitest';
import type { PromptWithVariants, Variant } from './types';
import { getPrimaryVariant } from './prompt';

function variant(id: string): Variant {
  return {
    id,
    prompt_id: 'prompt-1',
    label: id,
    content: `${id} content`,
    content_type: 'static',
    variables: null,
    sort_order: 0,
    created_at: null,
    updated_at: null,
  };
}

function prompt(primaryVariantId: string | null, variants: Variant[]): PromptWithVariants {
  return {
    id: 'prompt-1',
    title: 'Prompt',
    description: null,
    primary_variant_id: primaryVariantId,
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    variants,
    tags: [],
  };
}

describe('getPrimaryVariant', () => {
  it('returns the variant referenced by primary_variant_id', () => {
    const first = variant('first');
    const primary = variant('primary');

    expect(getPrimaryVariant(prompt(primary.id, [first, primary]))).toBe(primary);
  });

  it('falls back to the first variant when the primary reference is absent', () => {
    const first = variant('first');

    expect(getPrimaryVariant(prompt('missing', [first]))).toBe(first);
    expect(getPrimaryVariant(prompt(null, [first]))).toBe(first);
  });

  it('returns undefined when the prompt has no variants', () => {
    expect(getPrimaryVariant(prompt(null, []))).toBeUndefined();
  });
});
