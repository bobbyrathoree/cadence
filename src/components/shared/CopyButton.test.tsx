import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CopyButton } from './CopyButton';

const mocks = vi.hoisted(() => ({
  recordCopy: vi.fn(),
  showToast: vi.fn(),
  writeText: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: { prompts: { recordCopy: mocks.recordCopy } },
}));

vi.mock('../../lib/context', () => ({
  useAppContext: () => ({ showToast: mocks.showToast }),
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}));

describe('CopyButton failure visibility', () => {
  afterEach(cleanup);

  it('shows a failure toast when copy recording fails', async () => {
    mocks.recordCopy.mockRejectedValueOnce(new Error('copy failed'));
    render(
      <CopyButton
        content="Content"
        promptId="prompt-1"
        variantId="variant-1"
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Copy/ }));

    await waitFor(() =>
      expect(mocks.showToast).toHaveBeenCalledWith(
        expect.stringContaining("Couldn't copy prompt"),
        'error',
      ),
    );
    expect(mocks.writeText).not.toHaveBeenCalled();
  });
});
