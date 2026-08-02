import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppProvider } from '../../lib/context';
import { SettingsModal } from './SettingsModal';

const mocks = vi.hoisted(() => ({
  getApiEnabled: vi.fn(),
  setApiEnabled: vi.fn(),
}));

vi.mock('../../lib/api', () => ({
  api: {
    settings: {
      getApiEnabled: mocks.getApiEnabled,
      setApiEnabled: mocks.setApiEnabled,
    },
  },
}));

function renderSettings() {
  return render(
    <AppProvider>
      <SettingsModal
        isOpen
        onClose={() => undefined}
        shortcuts={[]}
        onUpdateShortcut={() => undefined}
        onResetAll={() => undefined}
      />
    </AppProvider>,
  );
}

describe('SettingsModal local API setting', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getApiEnabled.mockResolvedValue(false);
  });

  afterEach(cleanup);

  it('loads the setting and enables then disables through the lifecycle IPC adapter', async () => {
    mocks.setApiEnabled
      .mockResolvedValueOnce({ enabled: true, port: 41_237 })
      .mockResolvedValueOnce({ enabled: false, port: null });
    renderSettings();

    const toggle = await screen.findByRole('switch', {
      name: 'Enable local API',
    });
    await waitFor(() => expect(toggle).not.toBeDisabled());
    expect(toggle).not.toBeChecked();

    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(mocks.setApiEnabled).toHaveBeenNthCalledWith(1, true);

    fireEvent.click(toggle);
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(mocks.setApiEnabled).toHaveBeenNthCalledWith(2, false);
  });

  it('keeps the persisted value and surfaces enable failures inline', async () => {
    mocks.setApiEnabled.mockRejectedValueOnce(new Error('port unavailable'));
    renderSettings();

    const toggle = await screen.findByRole('switch', {
      name: 'Enable local API',
    });
    await waitFor(() => expect(toggle).not.toBeDisabled());
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent('port unavailable'),
    );
    expect(toggle).not.toBeChecked();
  });
});
