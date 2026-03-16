import { useEffect, useMemo } from 'react';
import type { KeyboardShortcut } from '../../lib/types';
import { ShortcutRecorder } from './ShortcutRecorder';

interface Props {
  isOpen: boolean;
  onClose: () => void;
  shortcuts: KeyboardShortcut[];
  onUpdateShortcut: (action: string, binding: string) => void;
  onResetAll: () => void;
}

export function SettingsModal({ isOpen, onClose, shortcuts, onUpdateShortcut, onResetAll }: Props) {
  // Escape key closes modal
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  const existingBindings = useMemo(
    () => new Map(shortcuts.filter((s) => s.binding).map((s) => [s.binding, s.action])),
    [shortcuts],
  );

  const globalShortcuts = useMemo(
    () => shortcuts.filter((s) => s.is_global),
    [shortcuts],
  );

  const appShortcuts = useMemo(
    () => shortcuts.filter((s) => !s.is_global),
    [shortcuts],
  );

  if (!isOpen) return null;

  return (
    // Backdrop
    <div
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 9000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0, 0, 0, 0.4)',
        backdropFilter: 'blur(4px)',
        WebkitBackdropFilter: 'blur(4px)',
        animation: 'modalFadeIn 0.15s ease-out',
      }}
    >
      {/* Modal card */}
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 550,
          maxHeight: '80vh',
          display: 'flex',
          flexDirection: 'column',
          background: 'var(--bg-secondary)',
          borderRadius: 12,
          boxShadow: '0 24px 80px rgba(0, 0, 0, 0.35), 0 0 0 1px rgba(255,255,255,0.05)',
          overflow: 'hidden',
          animation: 'modalSlideIn 0.2s ease-out',
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between flex-shrink-0"
          style={{
            padding: '16px 20px 12px',
            borderBottom: '1px solid var(--border)',
          }}
        >
          <h2
            style={{
              fontSize: '16px',
              fontWeight: 600,
              color: 'var(--text-primary)',
              margin: 0,
            }}
          >
            Keyboard Shortcuts
          </h2>
          <button
            onClick={onClose}
            style={{
              width: 28,
              height: 28,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 6,
              border: 'none',
              background: 'transparent',
              color: 'var(--text-secondary)',
              cursor: 'default',
              fontSize: '16px',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 15%, transparent)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
            }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <line x1="4" y1="4" x2="12" y2="12" />
              <line x1="12" y1="4" x2="4" y2="12" />
            </svg>
          </button>
        </div>

        {/* Scrollable list */}
        <div
          className="flex-1 overflow-y-auto"
          style={{ padding: '12px 20px 16px' }}
        >
          {/* Global section */}
          {globalShortcuts.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <div
                style={{
                  fontSize: '10px',
                  fontWeight: 600,
                  letterSpacing: '0.06em',
                  textTransform: 'uppercase',
                  color: 'var(--text-secondary)',
                  marginBottom: 8,
                }}
              >
                Global
              </div>
              {globalShortcuts.map((shortcut) => (
                <ShortcutRow
                  key={shortcut.action}
                  shortcut={shortcut}
                  existingBindings={existingBindings}
                  onUpdate={onUpdateShortcut}
                />
              ))}
            </div>
          )}

          {/* Application section */}
          {appShortcuts.length > 0 && (
            <div>
              <div
                style={{
                  fontSize: '10px',
                  fontWeight: 600,
                  letterSpacing: '0.06em',
                  textTransform: 'uppercase',
                  color: 'var(--text-secondary)',
                  marginBottom: 8,
                }}
              >
                Application
              </div>
              {appShortcuts.map((shortcut) => (
                <ShortcutRow
                  key={shortcut.action}
                  shortcut={shortcut}
                  existingBindings={existingBindings}
                  onUpdate={onUpdateShortcut}
                />
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div
          className="flex items-center flex-shrink-0"
          style={{
            padding: '12px 20px',
            borderTop: '1px solid var(--border)',
          }}
        >
          <button
            onClick={onResetAll}
            style={{
              padding: '6px 14px',
              fontSize: '12px',
              fontWeight: 500,
              color: 'var(--text-secondary)',
              background: 'transparent',
              border: '1px solid var(--border)',
              borderRadius: 6,
              cursor: 'default',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 10%, transparent)';
              e.currentTarget.style.color = 'var(--text-primary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.color = 'var(--text-secondary)';
            }}
          >
            Reset to Defaults
          </button>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Shortcut row                                                       */
/* ------------------------------------------------------------------ */

function ShortcutRow({
  shortcut,
  existingBindings,
  onUpdate,
}: {
  shortcut: KeyboardShortcut;
  existingBindings: Map<string, string>;
  onUpdate: (action: string, binding: string) => void;
}) {
  return (
    <div
      className="flex items-center justify-between"
      style={{
        padding: '8px 0',
        borderBottom: '1px solid color-mix(in srgb, var(--border) 50%, transparent)',
      }}
    >
      <div className="flex items-center gap-2">
        <span style={{ fontSize: '13px', color: 'var(--text-primary)' }}>
          {shortcut.label}
        </span>
        {shortcut.is_global && (
          <span
            style={{
              fontSize: '9px',
              fontWeight: 600,
              letterSpacing: '0.03em',
              padding: '1px 5px',
              borderRadius: 4,
              background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
              color: 'var(--accent)',
            }}
          >
            Global
          </span>
        )}
      </div>
      <ShortcutRecorder
        value={shortcut.binding}
        onChange={(binding) => onUpdate(shortcut.action, binding)}
        onClear={() => onUpdate(shortcut.action, '')}
        existingBindings={existingBindings}
        currentAction={shortcut.action}
      />
    </div>
  );
}
