import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { PlaybookWithSteps, PromptWithVariants } from '../../lib/types';
import { PlaybookStepper } from './PlaybookStepper';

const mocks = vi.hoisted(() => ({
  getPlaybook: vi.fn(),
  recordCopy: vi.fn(),
  advance: vi.fn(),
  writeText: vi.fn(),
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({ refreshCounter: 0 }),
}));

vi.mock('../../lib/hooks', () => ({
  usePlaybookSession: () => ({
    session: {
      active_playbook_id: 'playbook-1',
      current_step: 0,
      started_at: '2026-08-02T00:00:00Z',
    },
  }),
}));

vi.mock('../../lib/api', () => ({
  api: {
    playbooks: { get: mocks.getPlaybook },
    prompts: { recordCopy: mocks.recordCopy },
    session: {
      start: vi.fn(),
      end: vi.fn(),
      advance: mocks.advance,
    },
  },
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}));

function sourcePrompt(): PromptWithVariants {
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
        content: 'Copy this content',
        content_type: 'static',
        variables: null,
        sort_order: 0,
        created_at: null,
        updated_at: null,
      },
    ],
  };
}

describe('PlaybookStepper copy workflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    const prompt = sourcePrompt();
    const playbook: PlaybookWithSteps = {
      id: 'playbook-1',
      title: 'Playbook',
      description: null,
      steps: [
        {
          id: 'step-1',
          playbook_id: 'playbook-1',
          prompt_id: prompt.id,
          position: 0,
          step_type: 'single',
          instructions: null,
          choice_prompt_ids: [],
          prompt,
          choice_prompts: [],
        },
      ],
    };
    mocks.getPlaybook.mockResolvedValue(playbook);
    mocks.writeText.mockResolvedValue(undefined);
    mocks.recordCopy.mockResolvedValue('Copy this content');
    mocks.advance.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it('uses the clipboard plugin, records the copied variant, then advances', async () => {
    render(<PlaybookStepper playbookId="playbook-1" />);
    fireEvent.click(await screen.findByText('Copy Step 1'));

    await waitFor(() => expect(mocks.advance).toHaveBeenCalledTimes(1));
    expect(mocks.writeText).toHaveBeenCalledWith('Copy this content');
    expect(mocks.recordCopy).toHaveBeenCalledWith('prompt-1', 'variant-1');
    expect(mocks.writeText.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.recordCopy.mock.invocationCallOrder[0],
    );
    expect(mocks.recordCopy.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.advance.mock.invocationCallOrder[0],
    );
  });
});
