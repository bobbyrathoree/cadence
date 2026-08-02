import { useEffect, useRef } from 'react';
import { useAppContext } from '../../lib/context';

interface Props {
  id: string;
  isOpen: boolean;
  onClose: () => void;
  ariaLabel: string;
  children: React.ReactNode;
  width?: number | string;
  maxHeight?: number | string;
  panelStyle?: React.CSSProperties;
}

const FOCUSABLE_SELECTOR = [
  'button:not([disabled])',
  '[href]',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function Modal({
  id,
  isOpen,
  onClose,
  ariaLabel,
  children,
  width = 420,
  maxHeight,
  panelStyle,
}: Props) {
  const { registerModal, unregisterModal, isTopModal } = useAppContext();
  const panelRef = useRef<HTMLDivElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);
  const backdropPressRef = useRef(false);

  useEffect(() => {
    if (!isOpen) return;
    restoreFocusRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    registerModal(id);
    const frame = requestAnimationFrame(() => {
      const firstFocusable =
        panelRef.current?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR);
      (firstFocusable ?? panelRef.current)?.focus();
    });

    return () => {
      cancelAnimationFrame(frame);
      unregisterModal(id);
      restoreFocusRef.current?.focus();
      restoreFocusRef.current = null;
    };
  }, [id, isOpen, registerModal, unregisterModal]);

  useEffect(() => {
    if (!isOpen) return;

    function handleKeyDown(event: KeyboardEvent) {
      if (!isTopModal(id)) return;
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopImmediatePropagation();
        onClose();
        return;
      }
      if (event.key !== 'Tab' || !panelRef.current) return;

      const focusable = [
        ...panelRef.current.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
      ].filter((element) => element.offsetParent !== null);
      if (focusable.length === 0) {
        event.preventDefault();
        panelRef.current.focus();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [id, isOpen, isTopModal, onClose]);

  if (!isOpen) return null;

  return (
    <div
      data-modal-backdrop={id}
      onMouseDown={(event) => {
        backdropPressRef.current = event.target === event.currentTarget;
      }}
      onMouseUp={(event) => {
        const shouldClose =
          backdropPressRef.current && event.target === event.currentTarget;
        backdropPressRef.current = false;
        if (shouldClose && isTopModal(id)) onClose();
      }}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 9000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0, 0, 0, 0.45)',
        backdropFilter: 'blur(4px)',
        WebkitBackdropFilter: 'blur(4px)',
        animation: 'modalFadeIn 0.15s ease-out',
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        tabIndex={-1}
        style={{
          width,
          maxWidth: 'calc(100vw - 32px)',
          maxHeight,
          background: 'var(--bg-secondary)',
          border: '1px solid var(--border)',
          borderRadius: 8,
          boxShadow: '0 24px 80px rgba(0, 0, 0, 0.35)',
          outline: 'none',
          animation: 'modalSlideIn 0.2s ease-out',
          ...panelStyle,
        }}
      >
        {children}
      </div>
    </div>
  );
}
