import type { InvokeArgs } from '@tauri-apps/api/core';

interface CadenceE2eBridge {
  invoke: (command: string, args: Record<string, unknown>) => unknown;
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
  mockIPC(async <T>(command: string, payload?: InvokeArgs) => {
    return (await bridge.invoke(
      command,
      (payload ?? {}) as Record<string, unknown>,
    )) as T;
  });
}
