import type {
  Playbook,
  PlaybookStepWithPrompt,
  PlaybookWithSteps,
  StepSpec,
} from './types';

export interface PlaybookDraftStep {
  key: string;
  id: string | null;
  step_type: 'single' | 'choice';
  prompt_id: string | null;
  choice_prompt_ids: string[];
  instructions: string;
}

export interface PlaybookDraft {
  playbookId: string | null;
  title: string;
  description: string;
  steps: PlaybookDraftStep[];
  initialSteps: PlaybookDraftStep[];
}

export interface PlaybookDraftPersistence {
  createPlaybook: (title: string, description?: string) => Promise<Playbook>;
  updatePlaybook: (
    id: string,
    request: { title: string; description: string | null },
  ) => Promise<unknown>;
  addStep: (
    playbookId: string,
    spec: StepSpec,
  ) => Promise<PlaybookStepWithPrompt>;
  updateStep: (
    playbookId: string,
    stepId: string,
    spec: StepSpec,
  ) => Promise<unknown>;
  removeStep: (playbookId: string, stepId: string) => Promise<unknown>;
  reorderSteps: (
    playbookId: string,
    orderedStepIds: string[],
  ) => Promise<unknown>;
}

export interface PlaybookDraftProgress {
  playbookId: string;
  createdStepIds: Array<{ key: string; stepId: string }>;
}

export class PlaybookDraftPersistenceError extends Error {
  constructor(
    readonly originalError: unknown,
    readonly progress: PlaybookDraftProgress,
  ) {
    super(originalError instanceof Error ? originalError.message : String(originalError));
    this.name = 'PlaybookDraftPersistenceError';
  }
}

let draftStepSequence = 0;

export function createDraftStep(
  stepType: 'single' | 'choice',
  key = `draft-step-${++draftStepSequence}`,
  id: string | null = null,
): PlaybookDraftStep {
  return {
    key,
    id,
    step_type: stepType,
    prompt_id: null,
    choice_prompt_ids: [],
    instructions: '',
  };
}

export function createPlaybookDraft(
  playbook?: PlaybookWithSteps,
): PlaybookDraft {
  const steps = playbook
    ? [...playbook.steps]
        .sort((left, right) => left.position - right.position)
        .map(stepToDraft)
    : [];
  return {
    playbookId: playbook?.id ?? null,
    title: playbook?.title ?? '',
    description: playbook?.description ?? '',
    steps,
    initialSteps: steps.map(cloneDraftStep),
  };
}

export function toStepSpec(step: PlaybookDraftStep): StepSpec {
  return {
    step_type: step.step_type,
    prompt_id: step.step_type === 'single' ? step.prompt_id : null,
    choice_prompt_ids:
      step.step_type === 'choice' ? [...step.choice_prompt_ids] : [],
    instructions: step.instructions.trim() || null,
  };
}

export function validatePlaybookDraft(draft: PlaybookDraft): string[] {
  const errors: string[] = [];
  if (!draft.title.trim()) errors.push('Playbook name is required');
  draft.steps.forEach((step, index) => {
    if (step.step_type === 'single' && !step.prompt_id) {
      errors.push(`Step ${index + 1} needs a prompt`);
    }
    if (step.step_type === 'choice') {
      const distinct = new Set(step.choice_prompt_ids);
      if (step.choice_prompt_ids.length < 2 || distinct.size < 2) {
        errors.push(`Step ${index + 1} needs at least two distinct prompts`);
      }
    }
  });
  return errors;
}

export async function persistPlaybookDraft(
  draft: PlaybookDraft,
  persistence: PlaybookDraftPersistence,
): Promise<string> {
  if (!draft.playbookId) {
    const created = await persistence.createPlaybook(
      draft.title.trim(),
      draft.description.trim() || undefined,
    );
    const progress: PlaybookDraftProgress = {
      playbookId: created.id,
      createdStepIds: [],
    };
    try {
      for (const step of draft.steps) {
        const added = await persistence.addStep(created.id, toStepSpec(step));
        progress.createdStepIds.push({ key: step.key, stepId: added.id });
      }
    } catch (error) {
      throw new PlaybookDraftPersistenceError(error, progress);
    }
    return created.id;
  }

  const playbookId = draft.playbookId;
  await persistence.updatePlaybook(playbookId, {
    title: draft.title.trim(),
    description: draft.description.trim() || null,
  });

  const initialById = new Map(
    draft.initialSteps
      .filter((step): step is PlaybookDraftStep & { id: string } => Boolean(step.id))
      .map((step) => [step.id, step]),
  );
  const currentIds = new Set(
    draft.steps.map((step) => step.id).filter((id): id is string => Boolean(id)),
  );
  let structureChanged = false;

  for (const initial of initialById.values()) {
    if (!currentIds.has(initial.id)) {
      await persistence.removeStep(playbookId, initial.id);
      structureChanged = true;
    }
  }

  const orderedIds: string[] = [];
  for (const step of draft.steps) {
    if (!step.id) {
      const added = await persistence.addStep(playbookId, toStepSpec(step));
      orderedIds.push(added.id);
      structureChanged = true;
      continue;
    }

    const initial = initialById.get(step.id);
    if (!initial || !sameSpec(initial, step)) {
      await persistence.updateStep(playbookId, step.id, toStepSpec(step));
    }
    orderedIds.push(step.id);
  }

  const initialOrder = draft.initialSteps
    .map((step) => step.id)
    .filter((id): id is string => Boolean(id));
  if (
    structureChanged ||
    initialOrder.length !== orderedIds.length ||
    initialOrder.some((id, index) => id !== orderedIds[index])
  ) {
    await persistence.reorderSteps(playbookId, orderedIds);
  }

  return playbookId;
}

export function applyPlaybookDraftProgress(
  draft: PlaybookDraft,
  progress: PlaybookDraftProgress,
): PlaybookDraft {
  const createdByKey = new Map(
    progress.createdStepIds.map(({ key, stepId }) => [key, stepId]),
  );
  const steps = draft.steps.map((step) => {
    const createdId = createdByKey.get(step.key);
    return createdId ? { ...step, id: createdId } : cloneDraftStep(step);
  });
  const initialIds = new Set(
    draft.initialSteps
      .map((step) => step.id)
      .filter((id): id is string => Boolean(id)),
  );
  const initialSteps = draft.initialSteps.map(cloneDraftStep);
  for (const step of steps) {
    if (step.id && createdByKey.has(step.key) && !initialIds.has(step.id)) {
      initialSteps.push(cloneDraftStep(step));
      initialIds.add(step.id);
    }
  }

  return {
    ...draft,
    playbookId: progress.playbookId,
    steps,
    initialSteps,
  };
}

function stepToDraft(step: PlaybookStepWithPrompt): PlaybookDraftStep {
  return {
    key: step.id,
    id: step.id,
    step_type: step.step_type,
    prompt_id: step.prompt_id,
    choice_prompt_ids: [...step.choice_prompt_ids],
    instructions: step.instructions ?? '',
  };
}

function cloneDraftStep(step: PlaybookDraftStep): PlaybookDraftStep {
  return {
    ...step,
    choice_prompt_ids: [...step.choice_prompt_ids],
  };
}

function sameSpec(left: PlaybookDraftStep, right: PlaybookDraftStep): boolean {
  return JSON.stringify(toStepSpec(left)) === JSON.stringify(toStepSpec(right));
}
