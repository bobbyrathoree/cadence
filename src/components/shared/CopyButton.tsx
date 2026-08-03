import { useEffect, useRef, useState } from 'react';

interface Props {
  onClick: () => Promise<'copied' | 'error' | 'cancelled'>;
}

type ButtonState = 'idle' | 'pending' | 'copied' | 'error';

export function CopyButton({ onClick }: Props) {
  const [state, setState] = useState<ButtonState>('idle');
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  async function handleCopy() {
    if (state === 'pending') return;
    setState('pending');
    let result: 'copied' | 'error' | 'cancelled';
    try {
      result = await onClick();
    } catch {
      result = 'error';
    }
    if (!mountedRef.current) return;
    if (result === 'cancelled') {
      setState('idle');
      return;
    }
    setState(result);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      if (mountedRef.current) setState('idle');
    }, 1_500);
  }

  const label =
    state === 'pending'
      ? 'Copying...'
      : state === 'copied'
        ? 'Copied \u2713'
        : state === 'error'
          ? 'Copy failed'
          : 'Copy';

  return (
    <button
      onClick={() => void handleCopy()}
      disabled={state === 'pending'}
      className="flex items-center gap-1.5 rounded cursor-default outline-none"
      style={{
        padding: '6px 16px',
        fontSize: '12px',
        fontWeight: 500,
        border: 'none',
        background:
          state === 'copied'
            ? '#34c759'
            : state === 'error'
              ? '#ff453a'
              : 'var(--accent)',
        color: '#ffffff',
        borderRadius: 6,
        opacity: state === 'pending' ? 0.7 : 1,
        transition: 'background 0.15s ease',
      }}
    >
      {label}
      {state === 'idle' && (
        <kbd
          style={{
            fontSize: '10px',
            opacity: 0.7,
            fontFamily: 'inherit',
          }}
        >
          {'\u2318'}C
        </kbd>
      )}
    </button>
  );
}
