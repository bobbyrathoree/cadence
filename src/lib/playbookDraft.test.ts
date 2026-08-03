import { describe, expect, it, vi } from 'vitest';
import type { PlaybookStepWithPrompt } from './types';
import {
  applyPlaybookDraftProgress,
  createDraftStep,
  persistPlaybookDraft,
  type PlaybookDraft,
  PlaybookDraftPersistenceError,
  type PlaybookDraftPersistence,
} from './playbookDraft';

function hydratedStep(id: string, promptId: string): PlaybookStepWithPrompt {
  return {
    id,
    playbook_id: 'playbook-1',
    prompt_id: promptId,
    position: 0,
    step_type: 'single',
    instructions: null,
    choice_prompt_ids: [],
    prompt: null,
    choice_prompts: [],
  };
}

function persistence(): PlaybookDraftPersistence {
  return {
    createPlaybook: vi.fn().mockResolvedValue({
      id: 'playbook-1',
      title: 'Workflow',
      description: null,
    }),
    updatePlaybook: vi.fn().mockResolvedValue(undefined),
    addStep: vi
      .fn()
      .mockImplementation((_playbookId, spec) =>
        Promise.resolve(hydratedStep(`created-${spec.prompt_id}`, spec.prompt_id!)),
      ),
    updateStep: vi.fn().mockResolvedValue(undefined),
    removeStep: vi.fn().mockResolvedValue(undefined),
    reorderSteps: vi.fn().mockResolvedValue(undefined),
  };
}

describe('playbook draft persistence', () => {
  it('creates steps in displayed order with their complete specs', async () => {
    const first = createDraftStep('single', 'first');
    first.prompt_id = 'prompt-a';
    const second = createDraftStep('choice', 'second');
    second.choice_prompt_ids = ['prompt-b', 'prompt-c'];
    second.instructions = 'Choose one';
    const draft: PlaybookDraft = {
      playbookId: null,
      title: 'Workflow',
      description: '',
      steps: [first, second],
      initialSteps: [],
    };
    const adapter = persistence();

    const id = await persistPlaybookDraft(draft, adapter);

    expect(id).toBe('playbook-1');
    expect(adapter.addStep).toHaveBeenCalledTimes(2);
    expect(vi.mocked(adapter.addStep).mock.calls).toEqual([
      [
        'playbook-1',
        {
          step_type: 'single',
          prompt_id: 'prompt-a',
          choice_prompt_ids: [],
          instructions: null,
        },
      ],
      [
        'playbook-1',
        {
          step_type: 'choice',
          prompt_id: null,
          choice_prompt_ids: ['prompt-b', 'prompt-c'],
          instructions: 'Choose one',
        },
      ],
    ]);
    expect(adapter.reorderSteps).not.toHaveBeenCalled();
  });

  it('updates changed steps, removes deleted steps, and reorders exact persisted IDs', async () => {
    const first = createDraftStep('single', 'first', 'step-a');
    first.prompt_id = 'prompt-a';
    const second = createDraftStep('single', 'second', 'step-b');
    second.prompt_id = 'prompt-b';
    const added = createDraftStep('single', 'added');
    added.prompt_id = 'prompt-c';
    const initialFirst = { ...first };
    const initialSecond = { ...second };
    second.instructions = 'Updated';
    const draft: PlaybookDraft = {
      playbookId: 'playbook-1',
      title: 'Renamed',
      description: 'Description',
      steps: [second, added],
      initialSteps: [initialFirst, initialSecond],
    };
    const adapter = persistence();

    await persistPlaybookDraft(draft, adapter);

    expect(adapter.updatePlaybook).toHaveBeenCalledWith('playbook-1', {
      title: 'Renamed',
      description: 'Description',
    });
    expect(adapter.removeStep).toHaveBeenCalledWith('playbook-1', 'step-a');
    expect(adapter.updateStep).toHaveBeenCalledWith(
      'playbook-1',
      'step-b',
      expect.objectContaining({ instructions: 'Updated', prompt_id: 'prompt-b' }),
    );
    expect(adapter.reorderSteps).toHaveBeenCalledWith('playbook-1', [
      'step-b',
      'created-prompt-c',
    ]);
  });

  it('retries a partial create without creating a second playbook', async () => {
    const first = createDraftStep('single', 'first');
    first.prompt_id = 'prompt-a';
    const second = createDraftStep('single', 'second');
    second.prompt_id = 'prompt-b';
    const draft: PlaybookDraft = {
      playbookId: null,
      title: 'Workflow',
      description: '',
      steps: [first, second],
      initialSteps: [],
    };
    const adapter = persistence();
    vi.mocked(adapter.addStep)
      .mockResolvedValueOnce(hydratedStep('step-a', 'prompt-a'))
      .mockRejectedValueOnce(new Error('step 2 failed'))
      .mockResolvedValueOnce(hydratedStep('step-b', 'prompt-b'));

    let retryDraft: PlaybookDraft | null = null;
    try {
      await persistPlaybookDraft(draft, adapter);
    } catch (error) {
      expect(error).toBeInstanceOf(PlaybookDraftPersistenceError);
      const persistenceError = error as PlaybookDraftPersistenceError;
      expect(persistenceError.progress).toEqual({
        playbookId: 'playbook-1',
        createdStepIds: [{ key: 'first', stepId: 'step-a' }],
      });
      retryDraft = applyPlaybookDraftProgress(draft, persistenceError.progress);
    }

    expect(retryDraft).not.toBeNull();
    await persistPlaybookDraft(retryDraft!, adapter);

    expect(adapter.createPlaybook).toHaveBeenCalledTimes(1);
    expect(adapter.addStep).toHaveBeenCalledTimes(3);
    expect(vi.mocked(adapter.addStep).mock.calls.map((call) => call[1].prompt_id))
      .toEqual(['prompt-a', 'prompt-b', 'prompt-b']);
  });
});
