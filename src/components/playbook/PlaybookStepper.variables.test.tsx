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
  useAppContext: () => ({
    refreshCounter: 0,
    triggerRefresh: vi.fn(),
    setPlaybookBuilderMode: vi.fn(),
    showToast: vi.fn(),
    registerModal: vi.fn(),
    unregisterModal: vi.fn(),
    isTopModal: () => true,
  }),
}));

vi.mock('../../lib/hooks', () => ({
  usePlaybookSession: () => ({
    data: {
      active_playbook_id: 'playbook-1',
      current_step: 0,
      started_at: '2026-08-02T00:00:00Z',
    },
    error: null,
    loading: false,
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

describe('PlaybookStepper variable copy', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    const source = prompt();
    mocks.getPlaybook.mockResolvedValue({
      id: 'playbook-1',
      title: 'Playbook',
      description: null,
      steps: [
        {
          id: 'step-1',
          playbook_id: 'playbook-1',
          prompt_id: source.id,
          position: 0,
          step_type: 'single',
          instructions: null,
          choice_prompt_ids: [],
          prompt: source,
          choice_prompts: [],
        },
      ],
    } satisfies PlaybookWithSteps);
    mocks.writeText.mockResolvedValue(undefined);
    mocks.recordCopy.mockResolvedValue('');
    mocks.advance.mockResolvedValue(undefined);
  });

  afterEach(cleanup);

  it('advances exactly once after confirmation and never on cancel', async () => {
    render(<PlaybookStepper playbookId="playbook-1" />);
    fireEvent.click(await screen.findByText('Copy Step 1'));
    fireEvent.keyDown(await screen.findByLabelText('name'), { key: 'Escape' });
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Copy Step 1' })).toBeEnabled(),
    );
    expect(mocks.advance).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Copy Step 1' }));
    const field = await screen.findByLabelText('name');
    fireEvent.change(field, { target: { value: 'Ada' } });
    fireEvent.keyDown(field, { key: 'Enter' });

    await waitFor(() => expect(mocks.advance).toHaveBeenCalledTimes(1));
    expect(mocks.writeText).toHaveBeenCalledWith('Hello Ada');
  });
});
