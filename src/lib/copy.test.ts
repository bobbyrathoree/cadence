import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  canApplyCopyEffect,
  startCopy,
  type CopyOperation,
} from './copy';

const mocks = vi.hoisted(() => ({
  recordCopy: vi.fn(),
  writeText: vi.fn(),
}));

vi.mock('./api', () => ({
  api: { prompts: { recordCopy: mocks.recordCopy } },
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: mocks.writeText,
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe('CopyOperation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.writeText.mockResolvedValue(undefined);
    mocks.recordCopy.mockResolvedValue('');
  });

  it('settles immediate clipboard copy without awaiting accounting', async () => {
    const accounting = deferred<string>();
    const onAccounting = vi.fn();
    mocks.recordCopy.mockReturnValue(accounting.promise);
    const operation = startCopy({
      content: 'plain',
      promptId: 'prompt-1',
      variantId: 'variant-1',
      onAccounting,
    });

    expect(operation.fill).toBeNull();
    await expect(operation.settled).resolves.toEqual({ kind: 'copied' });
    expect(mocks.writeText).toHaveBeenCalledWith('plain');
    expect(mocks.recordCopy).toHaveBeenCalledWith('prompt-1', 'variant-1');
    expect(onAccounting).not.toHaveBeenCalled();

    accounting.resolve('plain');
    await accounting.promise;
    await vi.waitFor(() => expect(onAccounting).toHaveBeenCalledWith(
      true,
      operation.key,
    ));
  });

  it('defers every side effect until variables are resumed', async () => {
    const operation = startCopy({
      content: 'Hello {{name}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    expect(operation.fill?.names).toEqual(['name']);
    expect(mocks.writeText).not.toHaveBeenCalled();
    expect(mocks.recordCopy).not.toHaveBeenCalled();

    operation.fill?.resume(new Map([['name', 'Ada']]));
    await expect(operation.settled).resolves.toEqual({ kind: 'copied' });
    expect(mocks.writeText).toHaveBeenCalledWith('Hello Ada');
    expect(mocks.recordCopy).toHaveBeenCalledTimes(1);
  });

  it('cancels from filling with zero side effects and ignores duplicates', async () => {
    const operation = startCopy({
      content: '{{name}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    operation.fill?.cancel();
    operation.fill?.resume(new Map([['name', 'late']]));
    operation.fill?.cancel();

    await expect(operation.settled).resolves.toEqual({ kind: 'cancelled' });
    expect(mocks.writeText).not.toHaveBeenCalled();
    expect(mocks.recordCopy).not.toHaveBeenCalled();
  });

  it('ignores cancel after resume and settles the in-flight clipboard write', async () => {
    const clipboard = deferred<void>();
    mocks.writeText.mockReturnValue(clipboard.promise);
    const operation = startCopy({
      content: '{{name}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    operation.fill?.resume(new Map([['name', 'Ada']]));
    operation.fill?.cancel();
    operation.fill?.resume(new Map([['name', 'Grace']]));
    clipboard.resolve();

    await expect(operation.settled).resolves.toEqual({ kind: 'copied' });
    expect(mocks.writeText).toHaveBeenCalledTimes(1);
    expect(mocks.writeText).toHaveBeenCalledWith('Ada');
  });

  it('returns a bounded clipboard error and never dispatches accounting', async () => {
    mocks.writeText.mockRejectedValue(new Error('x'.repeat(250)));
    const operation = startCopy({
      content: 'plain',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    const result = await operation.settled;

    expect(result.kind).toBe('clipboard_error');
    if (result.kind === 'clipboard_error') {
      expect(result.message).toHaveLength(200);
    }
    expect(mocks.recordCopy).not.toHaveBeenCalled();
  });

  it('gates stale accounting and settled effects by key and liveness', () => {
    const first = startCopy({
      content: '{{a}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    const second = startCopy({
      content: '{{b}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });

    expect(canApplyCopyEffect(second, first.key, true)).toBe(false);
    expect(canApplyCopyEffect(second, second.key, false)).toBe(false);
    expect(canApplyCopyEffect(second, second.key, true)).toBe(true);
    first.fill?.cancel();
    second.fill?.cancel();
  });

  it('ignores stale accounting across sequential operations and after unmount', async () => {
    const firstAccounting = deferred<string>();
    const secondAccounting = deferred<string>();
    mocks.recordCopy
      .mockReturnValueOnce(firstAccounting.promise)
      .mockReturnValueOnce(secondAccounting.promise);
    const applied = vi.fn();
    let alive = true;
    let current: CopyOperation | null = null;

    const first = startCopy({
      content: 'first',
      promptId: 'prompt-1',
      variantId: 'variant-1',
      onAccounting: (ok, key) => {
        if (canApplyCopyEffect(current, key, alive)) applied(ok, key);
      },
    });
    current = first;
    await first.settled;

    const second = startCopy({
      content: 'second',
      promptId: 'prompt-2',
      variantId: 'variant-2',
      onAccounting: (ok, key) => {
        if (canApplyCopyEffect(current, key, alive)) applied(ok, key);
      },
    });
    current = second;
    await second.settled;

    firstAccounting.resolve('first');
    await firstAccounting.promise;
    await Promise.resolve();
    expect(applied).not.toHaveBeenCalled();

    alive = false;
    secondAccounting.resolve('second');
    await secondAccounting.promise;
    await Promise.resolve();
    expect(applied).not.toHaveBeenCalled();
  });

  it('leaves no site effects for copies settling after unmount or target change', async () => {
    const effects = vi.fn();
    const clipboard = deferred<void>();
    mocks.writeText.mockReturnValue(clipboard.promise);
    let current: CopyOperation | null = startCopy({
      content: 'plain',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    const immediate = current;
    let alive = false;
    void immediate.settled.then(() => {
      if (canApplyCopyEffect(current, immediate.key, alive)) effects();
    });
    clipboard.resolve();
    await expect(immediate.settled).resolves.toEqual({ kind: 'copied' });
    expect(effects).not.toHaveBeenCalled();

    const resumedClipboard = deferred<void>();
    mocks.writeText.mockReturnValue(resumedClipboard.promise);
    alive = true;
    current = startCopy({
      content: '{{name}}',
      promptId: 'prompt-1',
      variantId: 'variant-1',
    });
    const resumed = current;
    void resumed.settled.then(() => {
      if (canApplyCopyEffect(current, resumed.key, alive)) effects();
    });
    resumed.fill?.resume(new Map([['name', 'Ada']]));
    current = null;
    resumed.fill?.cancel();
    resumedClipboard.resolve();
    await expect(resumed.settled).resolves.toEqual({ kind: 'copied' });
    expect(effects).not.toHaveBeenCalled();
  });
});
