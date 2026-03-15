import { useEffect, useRef } from 'react';

interface Props {
  message: string;
  visible: boolean;
  onHide?: () => void;
}

/**
 * A brief notification pill at the bottom center of the screen.
 * Auto-hides after 1.5 seconds.
 */
export function Toast({ message, visible, onHide }: Props) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (visible && onHide) {
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        onHide();
      }, 1500);
    }

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [visible, onHide]);

  return (
    <div
      style={{
        position: 'fixed',
        bottom: 24,
        left: '50%',
        transform: visible
          ? 'translateX(-50%) translateY(0)'
          : 'translateX(-50%) translateY(16px)',
        opacity: visible ? 1 : 0,
        pointerEvents: visible ? 'auto' : 'none',
        transition: 'opacity 0.2s ease, transform 0.2s ease',
        zIndex: 9999,
        padding: '8px 20px',
        borderRadius: 20,
        background: 'rgba(30, 30, 34, 0.92)',
        color: '#ffffff',
        fontSize: 12,
        fontWeight: 500,
        boxShadow: '0 4px 16px rgba(0, 0, 0, 0.2)',
        whiteSpace: 'nowrap' as const,
        backdropFilter: 'blur(8px)',
        WebkitBackdropFilter: 'blur(8px)',
      }}
    >
      {message}
    </div>
  );
}
