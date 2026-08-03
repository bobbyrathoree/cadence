import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AppProvider } from '../../lib/context';
import { FillVariablesModal } from './FillVariablesModal';

describe('FillVariablesModal', () => {
  afterEach(cleanup);

  it('renders first-seen fields, a live preview, and confirms on Enter', () => {
    const onConfirm = vi.fn();
    render(
      <AppProvider>
        <FillVariablesModal
          id="fill-test"
          content="Hello {{name}} from {{place}} and {{name}}"
          names={['name', 'place']}
          onConfirm={onConfirm}
          onCancel={() => undefined}
        />
      </AppProvider>,
    );

    fireEvent.change(screen.getByLabelText('name'), {
      target: { value: 'Ada' },
    });
    fireEvent.change(screen.getByLabelText('place'), {
      target: { value: 'London' },
    });
    expect(screen.getByLabelText('Interpolated preview')).toHaveTextContent(
      'Hello Ada from London and Ada',
    );
    fireEvent.keyDown(screen.getByLabelText('place'), { key: 'Enter' });

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect([...onConfirm.mock.calls[0][0].entries()]).toEqual([
      ['name', 'Ada'],
      ['place', 'London'],
    ]);
  });

  it('cancels through Escape', () => {
    const onCancel = vi.fn();
    render(
      <AppProvider>
        <FillVariablesModal
          id="fill-test"
          content="{{name}}"
          names={['name']}
          onConfirm={() => undefined}
          onCancel={onCancel}
        />
      </AppProvider>,
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
