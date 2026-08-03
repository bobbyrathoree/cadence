import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { startCopy } from '../../lib/copy';
import { CopyButton } from './CopyButton';

const mocks = vi.hoisted(() => ({
  recordCopy: vi.fn(),
  writeText: vi.fn(),
  accounting: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: { prompts: { recordCopy: mocks.recordCopy } },
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}));

describe('CopyButton clipboard-first workflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.writeText.mockResolvedValue(undefined);
    mocks.recordCopy.mockRejectedValue(new Error('accounting failed'));
  });

  afterEach(cleanup);

  it('reports copied after clipboard success and surfaces accounting separately', async () => {
    render(
      <CopyButton
        onClick={() => {
          const operation = startCopy({
            content: 'Content',
            promptId: 'prompt-1',
            variantId: 'variant-1',
            onAccounting: mocks.accounting,
          });
          return operation.settled.then((result) =>
            result.kind === 'copied' ? 'copied' : 'error',
          );
        }}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Copy/ }));

    expect(await screen.findByRole('button', { name: /Copied/ })).toBeVisible();
    expect(mocks.writeText).toHaveBeenCalledWith('Content');
    expect(mocks.recordCopy).toHaveBeenCalledWith('prompt-1', 'variant-1');
    expect(mocks.writeText.mock.invocationCallOrder[0]).toBeLessThan(
      mocks.recordCopy.mock.invocationCallOrder[0],
    );
    await waitFor(() =>
      expect(mocks.accounting).toHaveBeenCalledWith(
        false,
        expect.objectContaining({
          promptId: 'prompt-1',
          variantId: 'variant-1',
        }),
      ),
    );
  });

  it('stays busy for the full unsettled operation window', async () => {
    let resolve!: (result: 'copied') => void;
    const settled = new Promise<'copied'>((resolvePromise) => {
      resolve = resolvePromise;
    });
    render(<CopyButton onClick={() => settled} />);

    fireEvent.click(screen.getByRole('button', { name: /Copy/ }));
    const pending = screen.getByRole('button', { name: 'Copying...' });
    expect(pending).toBeDisabled();

    resolve('copied');
    expect(await screen.findByRole('button', { name: /Copied/ })).toBeEnabled();
  });
});
