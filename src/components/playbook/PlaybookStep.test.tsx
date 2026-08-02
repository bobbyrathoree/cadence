import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  PlaybookStepWithPrompt,
  PromptWithVariants,
} from '../../lib/types';
import { PlaybookStep } from './PlaybookStep';

function prompt(
  id: string,
  title: string,
  primaryVariantId: string,
): PromptWithVariants {
  return {
    id,
    title,
    description: null,
    primary_variant_id: primaryVariantId,
    is_favorite: false,
    is_pinned: false,
    copy_count: 0,
    last_copied_at: null,
    created_at: null,
    updated_at: null,
    tags: [],
    variants: [
      {
        id: `${id}-fallback`,
        prompt_id: id,
        label: 'Fallback',
        content: `${id} fallback content`,
        content_type: 'static',
        variables: null,
        sort_order: 0,
        created_at: null,
        updated_at: null,
      },
      {
        id: primaryVariantId,
        prompt_id: id,
        label: 'Primary',
        content: `${id} primary content`,
        content_type: 'static',
        variables: null,
        sort_order: 1,
        created_at: null,
        updated_at: null,
      },
    ],
  };
}

function step(
  overrides: Partial<PlaybookStepWithPrompt>,
): PlaybookStepWithPrompt {
  return {
    id: 'step-1',
    playbook_id: 'playbook-1',
    prompt_id: null,
    position: 0,
    step_type: 'single',
    instructions: null,
    choice_prompt_ids: [],
    prompt: null,
    choice_prompts: [],
    ...overrides,
  };
}

describe('PlaybookStep copy targets', () => {
  afterEach(cleanup);

  it('reports the selected single-prompt variant identity with its content', () => {
    const source = prompt('prompt-a', 'Alpha', 'prompt-a-primary');
    const onCopy = vi.fn();
    render(
      <PlaybookStep
        step={step({ prompt_id: source.id, prompt: source })}
        status="active"
        stepNumber={1}
        isLast
        onCopy={onCopy}
      />,
    );

    fireEvent.click(screen.getByText('Copy Step 1'));
    expect(onCopy).toHaveBeenCalledWith({
      promptId: 'prompt-a',
      variantId: 'prompt-a-primary',
      content: 'prompt-a primary content',
    });
  });

  it('reports the chosen prompt and primary variant for choice steps', () => {
    const alpha = prompt('prompt-a', 'Alpha', 'prompt-a-primary');
    const beta = prompt('prompt-b', 'Beta', 'prompt-b-primary');
    const onCopy = vi.fn();
    render(
      <PlaybookStep
        step={step({
          step_type: 'choice',
          choice_prompts: [alpha, beta],
        })}
        status="active"
        stepNumber={1}
        isLast
        onCopy={onCopy}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Beta' }));
    expect(onCopy).toHaveBeenCalledWith({
      promptId: 'prompt-b',
      variantId: 'prompt-b-primary',
      content: 'prompt-b primary content',
    });
  });
});
