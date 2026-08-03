import type { PromptWithVariants, Variant } from './types';

export const PROMPT_EDIT_EXIT_EVENT = 'cadence:request-prompt-edit-exit';

export interface PromptEditExitDetail {
  afterExit: () => void;
}

export interface PromptMetadataDraft {
  title: string;
  description: string | null;
  originalTitle: string;
  originalDescription: string | null;
}

export interface VariantDraft {
  variantId: string;
  label: string;
  content: string;
  originalLabel: string;
  originalContent: string;
}

export interface PromptDraftState {
  promptId: string | null;
  metadata: PromptMetadataDraft | null;
  variants: Map<string, VariantDraft>;
}

export type PromptDraftAction =
  | {
      type: 'seed';
      prompt: PromptWithVariants;
      activeVariantId: string | null;
    }
  | {
      type: 'edit_metadata';
      title?: string;
      description?: string | null;
    }
  | {
      type: 'edit_variant';
      variantId: string;
      label?: string;
      content?: string;
    }
  | {
      type: 'mark_saved';
      metadata: boolean;
      variantIds: string[];
    }
  | { type: 'reset' };

export interface DraftPersistence {
  updatePrompt: (
    promptId: string,
    request: { title: string; description: string | null },
  ) => Promise<unknown>;
  updateVariant: (variantId: string, content: string, label: string) => Promise<unknown>;
}

export interface DraftSaveFailure {
  kind: 'metadata' | 'variant';
  label: string;
  variantId?: string;
  error: unknown;
}

export interface DraftSaveResult {
  savedMetadata: boolean;
  savedVariantIds: string[];
  failures: DraftSaveFailure[];
}

export function createPromptDraftState(): PromptDraftState {
  return {
    promptId: null,
    metadata: null,
    variants: new Map(),
  };
}

export function promptDraftReducer(
  state: PromptDraftState,
  action: PromptDraftAction,
): PromptDraftState {
  switch (action.type) {
    case 'seed':
      return seedDrafts(state, action.prompt, action.activeVariantId);
    case 'edit_metadata':
      if (!state.metadata) return state;
      return {
        ...state,
        metadata: {
          ...state.metadata,
          ...(action.title !== undefined ? { title: action.title } : {}),
          ...(action.description !== undefined ? { description: action.description } : {}),
        },
      };
    case 'edit_variant': {
      const draft = state.variants.get(action.variantId);
      if (!draft) return state;
      const variants = new Map(state.variants);
      variants.set(action.variantId, {
        ...draft,
        ...(action.label !== undefined ? { label: action.label } : {}),
        ...(action.content !== undefined ? { content: action.content } : {}),
      });
      return { ...state, variants };
    }
    case 'mark_saved': {
      const metadata =
        action.metadata && state.metadata
          ? {
              ...state.metadata,
              originalTitle: state.metadata.title,
              originalDescription: state.metadata.description,
            }
          : state.metadata;
      const variants = new Map(state.variants);
      for (const variantId of action.variantIds) {
        const draft = variants.get(variantId);
        if (draft) {
          variants.set(variantId, {
            ...draft,
            originalLabel: draft.label,
            originalContent: draft.content,
          });
        }
      }
      return { ...state, metadata, variants };
    }
    case 'reset':
      return createPromptDraftState();
  }
}

function seedDrafts(
  state: PromptDraftState,
  prompt: PromptWithVariants,
  activeVariantId: string | null,
): PromptDraftState {
  const current =
    state.promptId === prompt.id
      ? state
      : {
          promptId: prompt.id,
          metadata: null,
          variants: new Map<string, VariantDraft>(),
        };
  let metadata = current.metadata;
  let variants = current.variants;

  if (!metadata) {
    metadata = {
      title: prompt.title,
      description: prompt.description,
      originalTitle: prompt.title,
      originalDescription: prompt.description,
    };
  }

  if (activeVariantId && !variants.has(activeVariantId)) {
    const variant = prompt.variants.find((candidate) => candidate.id === activeVariantId);
    if (variant) {
      variants = new Map(variants);
      variants.set(variant.id, createVariantDraft(variant));
    }
  }

  if (metadata === current.metadata && variants === current.variants) {
    return current;
  }
  return {
    promptId: prompt.id,
    metadata,
    variants,
  };
}

function createVariantDraft(variant: Variant): VariantDraft {
  return {
    variantId: variant.id,
    label: variant.label,
    content: variant.content,
    originalLabel: variant.label,
    originalContent: variant.content,
  };
}

export function isMetadataDraftDirty(metadata: PromptMetadataDraft | null): boolean {
  return Boolean(
    metadata &&
      (metadata.title !== metadata.originalTitle ||
        metadata.description !== metadata.originalDescription),
  );
}

export function isVariantDraftDirty(draft: VariantDraft): boolean {
  return draft.label !== draft.originalLabel || draft.content !== draft.originalContent;
}

export function hasDirtyDrafts(state: PromptDraftState): boolean {
  return (
    isMetadataDraftDirty(state.metadata) ||
    [...state.variants.values()].some(isVariantDraftDirty)
  );
}

export async function savePromptDrafts(
  state: PromptDraftState,
  persistence: DraftPersistence,
): Promise<DraftSaveResult> {
  const result: DraftSaveResult = {
    savedMetadata: false,
    savedVariantIds: [],
    failures: [],
  };

  if (state.promptId && state.metadata && isMetadataDraftDirty(state.metadata)) {
    try {
      await persistence.updatePrompt(state.promptId, {
        title: state.metadata.title,
        description: state.metadata.description?.trim() || null,
      });
      result.savedMetadata = true;
    } catch (error) {
      result.failures.push({
        kind: 'metadata',
        label: 'prompt metadata',
        error,
      });
    }
  }

  for (const draft of state.variants.values()) {
    if (!isVariantDraftDirty(draft)) continue;
    try {
      await persistence.updateVariant(draft.variantId, draft.content, draft.label);
      result.savedVariantIds.push(draft.variantId);
    } catch (error) {
      result.failures.push({
        kind: 'variant',
        label: draft.label,
        variantId: draft.variantId,
        error,
      });
    }
  }

  return result;
}

export function describeDirtyDrafts(state: PromptDraftState): string {
  const metadataFields: string[] = [];
  if (state.metadata?.title !== state.metadata?.originalTitle) metadataFields.push('Title');
  if (state.metadata?.description !== state.metadata?.originalDescription) {
    metadataFields.push('Description');
  }
  const variantLabels = [...state.variants.values()]
    .filter(isVariantDraftDirty)
    .map((draft) => draft.label);

  if (metadataFields.length > 0 && variantLabels.length > 0) {
    return `${metadataFields.join(', ')}, and edits to variants: ${variantLabels.join(', ')}`;
  }
  if (variantLabels.length > 0) {
    return `Edits to variants: ${variantLabels.join(', ')}`;
  }
  return metadataFields.join(', ');
}
