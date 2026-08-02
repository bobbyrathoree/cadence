import type { InvokeArgs } from '@tauri-apps/api/core';

interface CadenceE2eBridge {
  invoke: (command: string, args: Record<string, unknown>) => unknown;
  emit: (event: string, payload?: unknown) => void;
}

declare global {
  interface Window {
    __CADENCE_E2E__?: CadenceE2eBridge;
  }
}

export async function installE2eMock(currentWindow: 'main' | 'search') {
  const bridge = window.__CADENCE_E2E__;
  if (!bridge) return;

  const { mockIPC, mockWindows } = await import('@tauri-apps/api/mocks');
  const otherWindow = currentWindow === 'main' ? 'search' : 'main';

  mockWindows(currentWindow, otherWindow);
  mockIPC(
    async (command: string, payload?: InvokeArgs) =>
      bridge.invoke(command, (payload ?? {}) as Record<string, unknown>),
    { shouldMockEvents: true },
  );

  const { emit } = await import('@tauri-apps/api/event');
  bridge.emit = (event, payload) => {
    void emit(event, payload);
  };
}
