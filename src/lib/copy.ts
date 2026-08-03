import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { api } from './api';
import { interpolate, parse, variableNames } from './variables';

export interface CopyKey {
  promptId: string;
  variantId: string;
  generation: number;
}

export interface StartCopyArgs {
  content: string;
  promptId: string;
  variantId: string;
  onAccounting?: (ok: boolean, key: CopyKey) => void;
}

export type CopyResult =
  | { kind: 'copied' }
  | { kind: 'clipboard_error'; message: string }
  | { kind: 'cancelled' };

export interface CopyOperation {
  settled: Promise<CopyResult>;
  fill: null | {
    names: string[];
    resume(values: Map<string, string>): void;
    cancel(): void;
  };
  key: CopyKey;
}

let nextGeneration = 1;

function sameKey(left: CopyKey, right: CopyKey): boolean {
  return (
    left.promptId === right.promptId &&
    left.variantId === right.variantId &&
    left.generation === right.generation
  );
}

export function canApplyCopyEffect(
  current: CopyOperation | null,
  key: CopyKey,
  alive: boolean,
): boolean {
  return alive && current !== null && sameKey(current.key, key);
}

function displayError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  return message.slice(0, 200);
}

export function startCopy(args: StartCopyArgs): CopyOperation {
  const segments = parse(args.content);
  const names = variableNames(segments);
  const key: CopyKey = {
    promptId: args.promptId,
    variantId: args.variantId,
    generation: nextGeneration,
  };
  nextGeneration += 1;

  let state: 'filling' | 'resolving' | 'settled' =
    names.length > 0 ? 'filling' : 'resolving';
  let settle!: (result: CopyResult) => void;
  const settled = new Promise<CopyResult>((resolve) => {
    settle = resolve;
  });

  function settleOnce(result: CopyResult) {
    if (state === 'settled') return;
    state = 'settled';
    settle(result);
  }

  function reportAccounting(ok: boolean) {
    try {
      args.onAccounting?.(ok, key);
    } catch (error) {
      console.error('Copy accounting callback failed', error);
    }
  }

  function dispatchAccounting() {
    let accounting: Promise<unknown>;
    try {
      accounting = api.prompts.recordCopy(args.promptId, args.variantId);
    } catch {
      reportAccounting(false);
      return;
    }
    void accounting.then(
      () => reportAccounting(true),
      () => reportAccounting(false),
    );
  }

  async function resolveCopy(values: Map<string, string>) {
    try {
      await writeText(interpolate(segments, values));
    } catch (error) {
      settleOnce({ kind: 'clipboard_error', message: displayError(error) });
      return;
    }
    settleOnce({ kind: 'copied' });
    dispatchAccounting();
  }

  const fill =
    names.length === 0
      ? null
      : {
          names,
          resume(values: Map<string, string>) {
            if (state !== 'filling') return;
            state = 'resolving';
            void resolveCopy(values);
          },
          cancel() {
            if (state !== 'filling') return;
            settleOnce({ kind: 'cancelled' });
          },
        };

  const operation: CopyOperation = { settled, fill, key };
  if (!fill) {
    void resolveCopy(new Map());
  }
  return operation;
}
