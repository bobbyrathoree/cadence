import type { PromptWithVariants, Variant } from './types';

export function getPrimaryVariant(prompt: PromptWithVariants): Variant | undefined {
  return (
    prompt.variants.find((variant) => variant.id === prompt.primary_variant_id) ??
    prompt.variants[0]
  );
}
