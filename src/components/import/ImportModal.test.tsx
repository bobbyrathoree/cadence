import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { AppProvider } from '../../lib/context';
import { ImportModal } from './ImportModal';

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
});
