import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppProvider } from '../../lib/context';
import { Modal } from './Modal';

function Harness({
  firstOpen = true,
  secondOpen = false,
  onFirstClose = vi.fn(),
  onSecondClose = vi.fn(),
}: {
  firstOpen?: boolean;
  secondOpen?: boolean;
  onFirstClose?: () => void;
  onSecondClose?: () => void;
}) {
  return (
    <AppProvider>
      <button type="button">Trigger</button>
      <Modal
        id="first"
        isOpen={firstOpen}
        onClose={onFirstClose}
        ariaLabel="First modal"
      >
        <button type="button">First action</button>
      </Modal>
      <Modal
        id="second"
        isOpen={secondOpen}
        onClose={onSecondClose}
        ariaLabel="Second modal"
      >
        <button type="button">Second action</button>
      </Modal>
    </AppProvider>
  );
}

describe('Modal', () => {
  afterEach(cleanup);

  it('sends Escape only to the topmost modal', async () => {
    const onFirstClose = vi.fn();
    const onSecondClose = vi.fn();
    render(
      <Harness
        secondOpen
        onFirstClose={onFirstClose}
        onSecondClose={onSecondClose}
      />,
    );

    await screen.findByRole('dialog', { name: 'Second modal' });
    fireEvent.keyDown(window, { key: 'Escape' });

    expect(onSecondClose).toHaveBeenCalledTimes(1);
    expect(onFirstClose).not.toHaveBeenCalled();
  });

  it('dismisses only when the pointer press and release both originate on the backdrop', async () => {
    const onClose = vi.fn();
    render(<Harness onFirstClose={onClose} />);
    const dialog = await screen.findByRole('dialog', { name: 'First modal' });
    const backdrop = dialog.parentElement!;

    fireEvent.mouseDown(backdrop);
    fireEvent.mouseUp(dialog);
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.mouseDown(backdrop);
    fireEvent.mouseUp(backdrop);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('restores focus to the opener when it closes', async () => {
    const { rerender } = render(<Harness firstOpen={false} />);
    const trigger = screen.getByRole('button', { name: 'Trigger' });
    trigger.focus();

    rerender(<Harness />);
    await screen.findByRole('dialog', { name: 'First modal' });
    rerender(<Harness firstOpen={false} />);

    await waitFor(() => expect(trigger).toHaveFocus());
  });
});
