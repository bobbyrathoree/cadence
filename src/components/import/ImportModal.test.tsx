import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppProvider } from '../../lib/context';
import { ImportModal } from './ImportModal';

const mocks = vi.hoisted(() => ({
  importJson: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: {
    importExport: {
      importJson: mocks.importJson,
      importMarkdownFiles: vi.fn(),
    },
    prompts: { create: vi.fn() },
  },
}));

describe('ImportModal state lifecycle', () => {
  afterEach(cleanup);

  it('keeps the slicer mounted across tabs and resets it on reopen', async () => {
    const { rerender } = render(
      <AppProvider>
        <ImportModal isOpen onClose={() => undefined} />
      </AppProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Prompt Slicer' }));
    const slicer = screen.getByPlaceholderText(/Paste your text content here/);
    fireEvent.change(slicer, { target: { value: 'Persistent slicer text' } });

    fireEvent.click(screen.getByRole('button', { name: 'JSON' }));
    fireEvent.click(screen.getByRole('button', { name: 'Prompt Slicer' }));
    expect(screen.getByText('Persistent slicer text')).toBeInTheDocument();

    rerender(
      <AppProvider>
        <ImportModal isOpen={false} onClose={() => undefined} />
      </AppProvider>,
    );
    rerender(
      <AppProvider>
        <ImportModal isOpen onClose={() => undefined} />
      </AppProvider>,
    );

    expect(screen.getByRole('button', { name: 'JSON' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Prompt Slicer' }));
    expect(screen.getByPlaceholderText(/Paste your text content here/)).toHaveValue('');
  });

  it('renders every import error returned by the backend', async () => {
    mocks.importJson.mockResolvedValueOnce({
      imported: 0,
      skipped: 1,
      errors: ['First row failed', 'Second row failed'],
    });
    render(
      <AppProvider>
        <ImportModal isOpen onClose={() => undefined} />
      </AppProvider>,
    );

    fireEvent.change(screen.getByPlaceholderText('Paste JSON here...'), {
      target: { value: '[{}]' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Import' }));

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('First row failed'),
    );
    expect(screen.getByRole('alert')).toHaveTextContent('Second row failed');
  });
});
