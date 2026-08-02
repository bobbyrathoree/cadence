import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PromptListItem as PromptListItemType } from '../../lib/types';
import { PromptListItem } from './PromptListItem';

describe('PromptListItem snippet runs', () => {
  afterEach(cleanup);

  it('rejoins astral-safe runs exactly and marks only highlighted text', () => {
    const item: PromptListItemType = {
      id: 'prompt-1',
      title: 'Music prompt',
      description: null,
      snippet: 'legacy fallback',
      snippet_runs: [
        { text: '\u{1f3b8}\u{1f3b8} ', highlighted: false },
        { text: 'guitar', highlighted: true },
        { text: ' riff', highlighted: false },
      ],
      is_favorite: false,
      variant_count: 1,
      copy_count: 0,
      last_copied_at: null,
      tags: [],
    };

    const { container } = render(
      <PromptListItem
        item={item}
        isSelected={false}
        onClick={vi.fn()}
      />,
    );

    const snippet = container.querySelector('button > div > div:nth-child(2)');
    expect(snippet).toHaveTextContent('\u{1f3b8}\u{1f3b8} guitar riff');
    expect(snippet?.textContent).toBe('\u{1f3b8}\u{1f3b8} guitar riff');
    expect(container.querySelector('mark')).toHaveTextContent('guitar');
  });
});
