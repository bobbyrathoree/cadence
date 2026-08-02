import { useEffect, useMemo, useState } from 'react';
import type { KeyboardShortcut } from '../../lib/types';
import {
  SETTINGS_SCHEMA,
  type ToggleSettingDefinition,
} from '../../lib/settings';
import { ShortcutRecorder } from './ShortcutRecorder';
import { Modal } from '../shared/Modal';

interface Props {
  isOpen: boolean;
  onClose: () => void;
  shortcuts: KeyboardShortcut[];
  onUpdateShortcut: (action: string, binding: string) => void;
  onResetAll: () => void;
}

export function SettingsModal({ isOpen, onClose, shortcuts, onUpdateShortcut, onResetAll }: Props) {
  const [settingValues, setSettingValues] = useState<Record<string, boolean>>({});
  const [settingsLoading, setSettingsLoading] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [updatingSettingId, setUpdatingSettingId] = useState<string | null>(null);
  const [settingsRetryCounter, setSettingsRetryCounter] = useState(0);

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

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setSettingsLoading(true);
    setSettingsError(null);

    Promise.all(
      SETTINGS_SCHEMA.map(async (definition) => [
        definition.id,
        await definition.read(),
      ] as const),
    )
      .then((entries) => {
        if (!cancelled) setSettingValues(Object.fromEntries(entries));
      })
      .catch((error) => {
        if (!cancelled) {
          setSettingsError(`Couldn't load settings: ${String(error)}`);
        }
      })
      .finally(() => {
        if (!cancelled) setSettingsLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen, settingsRetryCounter]);

  async function updateToggle(
    definition: ToggleSettingDefinition,
    enabled: boolean,
  ) {
    if (updatingSettingId) return;
    setUpdatingSettingId(definition.id);
    setSettingsError(null);
    try {
      const persisted = await definition.write(enabled);
      setSettingValues((current) => ({
        ...current,
        [definition.id]: persisted,
      }));
    } catch (error) {
      setSettingsError(`Couldn't update setting: ${String(error)}`);
    } finally {
      setUpdatingSettingId(null);
    }
  }

  return (
    <Modal
      id="settings"
      isOpen={isOpen}
      onClose={onClose}
      ariaLabel="Settings"
      width={550}
      maxHeight="80vh"
      panelStyle={{
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: 0,
          overflow: 'hidden',
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
            Settings
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
          {SETTINGS_SCHEMA.map((definition, index) => (
            <div
              key={definition.id}
              style={{ marginBottom: index === SETTINGS_SCHEMA.length - 1 ? 20 : 16 }}
            >
              <div style={sectionHeadingStyle}>{definition.section}</div>
              <ToggleSettingRow
                definition={definition}
                value={settingValues[definition.id] ?? false}
                disabled={
                  settingsLoading ||
                  updatingSettingId !== null
                }
                onChange={(enabled) => void updateToggle(definition, enabled)}
              />
            </div>
          ))}

          {settingsLoading && (
            <div
              role="status"
              style={{ marginTop: -12, marginBottom: 16, fontSize: 11, color: 'var(--text-secondary)' }}
            >
              Loading settings...
            </div>
          )}
          {settingsError && (
            <div
              role="alert"
              style={{
                marginTop: -12,
                marginBottom: 16,
                padding: '8px 10px',
                borderRadius: 6,
                color: '#ff453a',
                background: 'color-mix(in srgb, #ff453a 8%, transparent)',
                fontSize: 11,
              }}
            >
              {settingsError}
              {settingsError.startsWith("Couldn't load") && (
                <button
                  type="button"
                  onClick={() => setSettingsRetryCounter((counter) => counter + 1)}
                  style={{
                    marginLeft: 8,
                    border: 0,
                    padding: 0,
                    background: 'transparent',
                    color: '#ff453a',
                    fontWeight: 600,
                  }}
                >
                  Retry
                </button>
              )}
            </div>
          )}

          {/* Global section */}
          {globalShortcuts.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <div style={sectionHeadingStyle}>
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
              <div style={sectionHeadingStyle}>
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
    </Modal>
  );
}

function ToggleSettingRow({
  definition,
  value,
  disabled,
  onChange,
}: {
  definition: ToggleSettingDefinition;
  value: boolean;
  disabled: boolean;
  onChange: (enabled: boolean) => void;
}) {
  const descriptionId = `setting-${definition.id}-description`;

  return (
    <div
      className="flex items-start justify-between gap-5"
      style={{
        padding: '10px 0',
        borderBottom: '1px solid color-mix(in srgb, var(--border) 50%, transparent)',
      }}
    >
      <div>
        <div style={{ fontSize: 13, color: 'var(--text-primary)' }}>
          {definition.label}
        </div>
        <div
          id={descriptionId}
          style={{
            maxWidth: 390,
            marginTop: 4,
            fontSize: 11,
            lineHeight: 1.45,
            color: 'var(--text-secondary)',
          }}
        >
          {definition.description}
        </div>
      </div>
      <button
        type="button"
        role="switch"
        aria-label={definition.label}
        aria-checked={value}
        aria-describedby={descriptionId}
        disabled={disabled}
        onClick={() => onChange(!value)}
        style={{
          width: 36,
          height: 20,
          flexShrink: 0,
          border: 0,
          borderRadius: 10,
          padding: 2,
          marginTop: 1,
          background: value ? 'var(--accent)' : 'var(--border)',
          opacity: disabled ? 0.55 : 1,
        }}
      >
        <span
          style={{
            display: 'block',
            width: 16,
            height: 16,
            borderRadius: '50%',
            background: '#ffffff',
            transform: value ? 'translateX(16px)' : 'translateX(0)',
            transition: 'transform 0.15s ease',
          }}
        />
      </button>
    </div>
  );
}

const sectionHeadingStyle: React.CSSProperties = {
  fontSize: '10px',
  fontWeight: 600,
  letterSpacing: '0.06em',
  textTransform: 'uppercase',
  color: 'var(--text-secondary)',
  marginBottom: 8,
};

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
