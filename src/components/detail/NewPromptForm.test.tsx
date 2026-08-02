import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NewPromptForm } from './NewPromptForm';

const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  showToast: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: { prompts: { create: mocks.create } },
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({
    setIsCreating: vi.fn(),
    setSelectedPromptId: vi.fn(),
    showToast: mocks.showToast,
  }),
}));

describe('NewPromptForm failure visibility', () => {
  afterEach(cleanup);

  it('shows a failure toast when save fails', async () => {
    mocks.create.mockRejectedValueOnce(new Error('write failed'));
    render(<NewPromptForm />);

    fireEvent.change(screen.getByPlaceholderText('Prompt title'), {
      target: { value: 'Title' },
    });
    fireEvent.change(
      screen.getByPlaceholderText('Write your prompt content here...'),
      { target: { value: 'Content' } },
    );
    fireEvent.click(screen.getByRole('button', { name: /Save/ }));

    await waitFor(() =>
      expect(mocks.showToast).toHaveBeenCalledWith(
        expect.stringContaining("Couldn't save prompt"),
        'error',
      ),
    );
  });
});
