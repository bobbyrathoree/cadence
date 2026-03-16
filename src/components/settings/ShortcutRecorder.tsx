import { useState, useEffect, useCallback, useRef } from 'react';

interface Props {
  value: string;
  onChange: (binding: string) => void;
  onClear: () => void;
  existingBindings: Map<string, string>;
  currentAction: string;
}

/** Format a Tauri accelerator key part for macOS display */
function formatKeyPart(part: string): string {
  switch (part) {
    case 'CommandOrControl': return 'Cmd';
    case 'Shift': return 'Shift';
    case 'Alt': return 'Opt';
    default: return part;
  }
}

/** Parse a binding string into display parts */
function parseBinding(binding: string): string[] {
  if (!binding) return [];
  return binding.split('+').map(formatKeyPart);
}

/** Build a combo string from a keyboard event */
function eventToCombo(e: KeyboardEvent): { combo: string; hasNonModifier: boolean } {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('CommandOrControl');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');

  const key = e.key;
  const isModifier = ['Meta', 'Control', 'Shift', 'Alt'].includes(key);

  if (!isModifier) {
    const normalized =
      key === ',' ? 'Comma' :
      key === '.' ? 'Period' :
      key === ' ' ? 'Space' :
      key === 'ArrowUp' ? 'Up' :
      key === 'ArrowDown' ? 'Down' :
      key === 'ArrowLeft' ? 'Left' :
      key === 'ArrowRight' ? 'Right' :
      key.length === 1 ? key.toUpperCase() : key;
    parts.push(normalized);
  }

  return { combo: parts.join('+'), hasNonModifier: !isModifier };
}

export function ShortcutRecorder({ value, onChange, onClear, existingBindings, currentAction }: Props) {
  const [recording, setRecording] = useState(false);
  const [currentKeys, setCurrentKeys] = useState<string[]>([]);
  const [conflict, setConflict] = useState<string | null>(null);
  const conflictTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const recorderRef = useRef<HTMLDivElement>(null);

  const stopRecording = useCallback(() => {
    setRecording(false);
    setCurrentKeys([]);
  }, []);

  // Click outside to cancel recording
  useEffect(() => {
    if (!recording) return;
    function handleClick(e: MouseEvent) {
      if (recorderRef.current && !recorderRef.current.contains(e.target as Node)) {
        stopRecording();
      }
    }
    window.addEventListener('mousedown', handleClick);
    return () => window.removeEventListener('mousedown', handleClick);
  }, [recording, stopRecording]);

  // Key listeners while recording
  useEffect(() => {
    if (!recording) return;

    function handleKeyDown(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();

      // Escape cancels recording
      if (e.key === 'Escape') {
        stopRecording();
        return;
      }

      // Backspace clears the shortcut
      if (e.key === 'Backspace') {
        onClear();
        stopRecording();
        return;
      }

      const { combo, hasNonModifier } = eventToCombo(e);

      // Show current keys being held
      setCurrentKeys(parseBinding(combo));

      if (hasNonModifier) {
        // Check for conflicts
        const conflictAction = existingBindings.get(combo);
        if (conflictAction && conflictAction !== currentAction) {
          setConflict(conflictAction);
          if (conflictTimerRef.current) clearTimeout(conflictTimerRef.current);
          conflictTimerRef.current = setTimeout(() => {
            setConflict(null);
          }, 1500);
          stopRecording();
          return;
        }

        onChange(combo);
        stopRecording();
      }
    }

    function handleKeyUp(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
    }

    // Use capture phase so we intercept before App.tsx handler
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);
    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [recording, existingBindings, currentAction, onChange, onClear, stopRecording]);

  // Cleanup conflict timer
  useEffect(() => {
    return () => {
      if (conflictTimerRef.current) clearTimeout(conflictTimerRef.current);
    };
  }, []);

  const displayParts = recording ? currentKeys : parseBinding(value);

  return (
    <div
      ref={recorderRef}
      onClick={() => {
        if (!recording) {
          setConflict(null);
          setRecording(true);
          setCurrentKeys([]);
        }
      }}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: '4px 8px',
        minWidth: 80,
        minHeight: 28,
        borderRadius: 6,
        border: `1px solid ${
          conflict ? '#ff453a' :
          recording ? 'var(--accent)' :
          'var(--border)'
        }`,
        background: recording
          ? 'color-mix(in srgb, var(--accent) 8%, transparent)'
          : 'color-mix(in srgb, var(--text-secondary) 5%, transparent)',
        cursor: 'default',
        transition: 'border-color 0.15s ease, background 0.15s ease, box-shadow 0.15s ease',
        boxShadow: recording
          ? '0 0 0 2px color-mix(in srgb, var(--accent) 20%, transparent)'
          : conflict
            ? '0 0 0 2px color-mix(in srgb, #ff453a 20%, transparent)'
            : 'none',
      }}
    >
      {conflict ? (
        <span style={{ fontSize: '10px', color: '#ff453a', fontWeight: 500, whiteSpace: 'nowrap' }}>
          Used by {conflict}
        </span>
      ) : recording && displayParts.length === 0 ? (
        <span
          style={{
            fontSize: '11px',
            color: 'var(--accent)',
            fontWeight: 500,
            animation: 'stepPulse 1.5s ease-in-out infinite',
          }}
        >
          Press keys...
        </span>
      ) : displayParts.length > 0 ? (
        displayParts.map((part, i) => (
          <span
            key={i}
            style={{
              display: 'inline-block',
              padding: '1px 6px',
              fontSize: '11px',
              fontWeight: 500,
              color: 'var(--text-primary)',
              background: 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
              borderRadius: 4,
              lineHeight: '18px',
            }}
          >
            {part}
          </span>
        ))
      ) : (
        <span style={{ fontSize: '11px', color: 'var(--text-secondary)', fontStyle: 'italic' }}>
          None
        </span>
      )}
    </div>
  );
}
