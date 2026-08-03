import { useEffect, useMemo, useState } from 'react';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { KeyboardShortcut } from '../../lib/types';
import type { McpBinaryLocation } from '../../lib/types';
import { api } from '../../lib/api';
import { renderMcpSnippets } from '../../lib/mcpSettings';
import {
  SETTINGS_SCHEMA,
  type ToggleSettingDefinition,
} from '../../lib/settings';
import { ShortcutRecorder } from './ShortcutRecorder';
import { Modal } from '../shared/Modal';

interface Props {
  isOpen: boolean;
  onClose: () => void;
  apiError?: string | null;
  shortcuts: KeyboardShortcut[];
  onUpdateShortcut: (action: string, binding: string) => void;
  onResetAll: () => void;
}

export function SettingsModal({
  isOpen,
  onClose,
  apiError = null,
  shortcuts,
  onUpdateShortcut,
  onResetAll,
}: Props) {
  const [settingValues, setSettingValues] = useState<Record<string, boolean>>({});
  const [settingsLoading, setSettingsLoading] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [updatingSettingId, setUpdatingSettingId] = useState<string | null>(null);
  const [settingsRetryCounter, setSettingsRetryCounter] = useState(0);
  const [mcpLocation, setMcpLocation] = useState<McpBinaryLocation | null>(null);
  const [mcpError, setMcpError] = useState<string | null>(null);

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
        if (!cancelled) {
          setSettingValues(Object.fromEntries(entries));
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setSettingsError(`Couldn't load settings: ${String(error)}`);
        }
      })
      .finally(() => {
        if (!cancelled) setSettingsLoading(false);
      });

    api.settings
      .getMcpBinaryLocation()
      .then((location) => {
        if (!cancelled) {
          setMcpLocation(location);
          setMcpError(null);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setMcpError(`Couldn't resolve cadence-mcp: ${String(error)}`);
        }
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
              {definition.id === 'api_enabled' && apiError && (
                <div
                  role="alert"
                  style={{
                    marginTop: 6,
                    padding: '8px 10px',
                    borderRadius: 6,
                    color: '#ff453a',
                    background:
                      'color-mix(in srgb, #ff453a 8%, transparent)',
                    fontSize: 11,
                  }}
                >
                  {apiError}
                </div>
              )}
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

          <McpSettingsPane location={mcpLocation} error={mcpError} />

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

function McpSettingsPane({
  location,
  error,
}: {
  location: McpBinaryLocation | null;
  error: string | null;
}) {
  const snippets = useMemo(
    () => location ? renderMcpSnippets(location.path) : [],
    [location],
  );
  const [selectedId, setSelectedId] = useState('claude-code');
  const [copyStatus, setCopyStatus] = useState<string | null>(null);
  const selected = snippets.find((snippet) => snippet.id === selectedId) ?? snippets[0];

  async function copy(label: string, text: string) {
    try {
      await writeText(text);
      setCopyStatus(`${label} copied`);
    } catch (copyError) {
      setCopyStatus(`Couldn't copy: ${String(copyError)}`);
    }
  }

  return (
    <section style={{ marginBottom: 20 }} aria-labelledby="mcp-settings-heading">
      <div id="mcp-settings-heading" style={sectionHeadingStyle}>
        Model Context Protocol
      </div>
      {error && (
        <div role="alert" style={{ fontSize: 11, color: '#ff453a' }}>
          {error}
        </div>
      )}
      {!error && !location && (
        <div role="status" style={{ fontSize: 11, color: 'var(--text-secondary)' }}>
          Resolving cadence-mcp...
        </div>
      )}
      {location && selected && (
        <>
          {location.development && (
            <div
              role="note"
              style={{
                marginBottom: 8,
                padding: '8px 10px',
                borderRadius: 6,
                color: '#ff9f0a',
                background: 'color-mix(in srgb, #ff9f0a 10%, transparent)',
                fontSize: 11,
              }}
            >
              Development build: build cadence-mcp at the target path before registering it.
            </div>
          )}
          <div
            style={{
              marginBottom: 10,
              fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
              fontSize: 10,
              lineHeight: 1.45,
              color: 'var(--text-secondary)',
              overflowWrap: 'anywhere',
            }}
          >
            {location.path}
          </div>
          <label
            htmlFor="mcp-client"
            style={{ display: 'block', marginBottom: 5, fontSize: 11, color: 'var(--text-secondary)' }}
          >
            Client
          </label>
          <select
            id="mcp-client"
            value={selected.id}
            onChange={(event) => setSelectedId(event.target.value)}
            style={{
              width: '100%',
              height: 30,
              marginBottom: 10,
              padding: '0 8px',
              border: '1px solid var(--border)',
              borderRadius: 6,
              color: 'var(--text-primary)',
              background: 'var(--surface)',
              fontSize: 12,
            }}
          >
            {snippets.map((snippet) => (
              <option key={snippet.id} value={snippet.id}>
                {snippet.label}
              </option>
            ))}
          </select>
          <SnippetRow
            label="Registration"
            text={selected.registration}
            onCopy={() => void copy('Registration', selected.registration)}
          />
          <SnippetRow
            label="Enable MCP writes"
            text={selected.writesEnabled}
            onCopy={() => void copy('Writes configuration', selected.writesEnabled)}
          />
          {copyStatus && (
            <div role="status" style={{ marginTop: 6, fontSize: 10, color: 'var(--text-secondary)' }}>
              {copyStatus}
            </div>
          )}
          <p style={{ margin: '8px 0 0', fontSize: 11, lineHeight: 1.45, color: 'var(--text-secondary)' }}>
            register, then run your client's MCP list command (e.g. /mcp in Claude Code)
          </p>
        </>
      )}
    </section>
  );
}

function SnippetRow({
  label,
  text,
  onCopy,
}: {
  label: string;
  text: string;
  onCopy: () => void;
}) {
  return (
    <div style={{ marginBottom: 9 }}>
      <div className="flex items-center justify-between" style={{ marginBottom: 4 }}>
        <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>{label}</span>
        <button
          type="button"
          onClick={onCopy}
          aria-label={`Copy ${label}`}
          style={{
            border: 0,
            padding: '2px 4px',
            background: 'transparent',
            color: 'var(--accent)',
            fontSize: 10,
          }}
        >
          Copy
        </button>
      </div>
      <pre
        style={{
          margin: 0,
          padding: '8px 10px',
          border: '1px solid var(--border)',
          borderRadius: 6,
          overflowX: 'auto',
          whiteSpace: 'pre-wrap',
          overflowWrap: 'anywhere',
          fontSize: 10,
          lineHeight: 1.45,
          color: 'var(--text-primary)',
          background: 'color-mix(in srgb, var(--border) 20%, transparent)',
        }}
      >
        {text}
      </pre>
    </div>
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
