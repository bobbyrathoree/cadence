import React, {
  useState,
  useEffect,
  useMemo,
  useCallback,
  useRef,
  useReducer,
} from 'react';
import { usePromptDetail } from '../../lib/hooks';
import { useAppContext } from '../../lib/context';
import { api } from '../../lib/api';
import { getPrimaryVariant } from '../../lib/prompt';
import {
  createPromptDraftState,
  describeDirtyDrafts,
  hasDirtyDrafts,
  PROMPT_EDIT_EXIT_EVENT,
  type PromptEditExitDetail,
  promptDraftReducer,
  savePromptDrafts,
} from '../../lib/promptDrafts';
import { VariantSelector } from './VariantSelector';
import { TagPills } from './TagPills';
import { CopyButton } from '../shared/CopyButton';

interface Props {
  promptId: string;
}

/**
 * Highlight template variables in prompt content.
 * Mustache-style vars get blue highlights, bracket placeholders get orange.
 */
function highlightVariables(content: string): React.ReactNode[] {
  const pattern = /(\{\{[^}]+\}\}|\[[A-Z][A-Z _]*\])/g;
  const parts: React.ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(content)) !== null) {
    if (match.index > lastIndex) {
      parts.push(content.slice(lastIndex, match.index));
    }

    const token = match[0];
    const isMustache = token.startsWith('{{');

    parts.push(
      <span
        key={`${match.index}-${token}`}
        className="rounded px-1"
        style={{
          background: isMustache
            ? 'color-mix(in srgb, #007aff 18%, transparent)'
            : 'color-mix(in srgb, #ff9500 18%, transparent)',
          color: isMustache ? '#4dabff' : '#ffb84d',
          fontWeight: 500,
        }}
      >
        {token}
      </span>,
    );

    lastIndex = match.index + token.length;
  }

  if (lastIndex < content.length) {
    parts.push(content.slice(lastIndex));
  }

  return parts;
}

function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return 'Never';
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);

  if (diffMin < 1) return 'Just now';
  if (diffMin < 60) return `${diffMin}m ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDays = Math.floor(diffHr / 24);
  if (diffDays < 30) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

function formatDate(dateStr: string | null): string {
  if (!dateStr) return '--';
  return new Date(dateStr).toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  });
}

export function PromptDetail({ promptId }: Props) {
  const { isEditing, setIsEditing, refreshCounter } = useAppContext();
  const { prompt, loading } = usePromptDetail(promptId, refreshCounter);
  const [selectedVariantId, setSelectedVariantId] = useState<string | null>(null);

  // Edit-mode draft state
  const [drafts, dispatchDraft] = useReducer(
    promptDraftReducer,
    undefined,
    createPromptDraftState,
  );
  const [saving, setSaving] = useState(false);
  const [saveErrors, setSaveErrors] = useState<string[]>([]);
  const [showExitConfirm, setShowExitConfirm] = useState(false);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const selectedPromptRef = useRef<string | null>(null);
  const pendingExitActionRef = useRef<(() => void) | null>(null);

  // Reset selection only when the prompt ID changes, not on background refetches.
  useEffect(() => {
    if (prompt?.id === promptId && selectedPromptRef.current !== promptId) {
      selectedPromptRef.current = promptId;
      setSelectedVariantId(getPrimaryVariant(prompt)?.id ?? null);
      dispatchDraft({ type: 'reset' });
      setSaveErrors([]);
      setShowExitConfirm(false);
      pendingExitActionRef.current = null;
    }
  }, [promptId, prompt]);

  const selectedVariant = useMemo(() => {
    if (!prompt || !selectedVariantId) return null;
    return prompt.variants.find((v) => v.id === selectedVariantId) ?? prompt.variants[0] ?? null;
  }, [prompt, selectedVariantId]);

  // Seed only missing draft records. Same-prompt refetches preserve existing drafts.
  useEffect(() => {
    if (isEditing && prompt?.id === promptId && selectedVariant) {
      dispatchDraft({
        type: 'seed',
        prompt,
        activeVariantId: selectedVariant.id,
      });
      requestAnimationFrame(() => titleInputRef.current?.focus());
    }
  }, [isEditing, promptId, prompt, selectedVariant]);

  const highlightedContent = useMemo(() => {
    if (!selectedVariant) return [];
    return highlightVariables(selectedVariant.content);
  }, [selectedVariant]);

  const activeVariantDraft = selectedVariantId
    ? drafts.variants.get(selectedVariantId) ?? null
    : null;
  const dirty = hasDirtyDrafts(drafts);
  const draftsAreValid = Boolean(
    drafts.metadata?.title.trim() &&
      [...drafts.variants.values()].every(
        (draft) => draft.label.trim() && draft.content.trim(),
      ),
  );

  const handleSave = useCallback(async (): Promise<boolean> => {
    if (!prompt || !draftsAreValid) return false;

    setSaving(true);
    setSaveErrors([]);
    try {
      const result = await savePromptDrafts(drafts, {
        updatePrompt: (id, request) => api.prompts.update(id, request),
        updateVariant: (id, content, label) => api.variants.update(id, content, label),
      });
      dispatchDraft({
        type: 'mark_saved',
        metadata: result.savedMetadata,
        variantIds: result.savedVariantIds,
      });

      if (result.failures.length > 0) {
        setSaveErrors(
          result.failures.map(
            (failure) => `Failed to save ${failure.label}: ${String(failure.error)}`,
          ),
        );
        return false;
      }

      dispatchDraft({ type: 'reset' });
      setShowExitConfirm(false);
      setIsEditing(false);
      pendingExitActionRef.current?.();
      pendingExitActionRef.current = null;
      return true;
    } finally {
      setSaving(false);
    }
  }, [prompt, drafts, draftsAreValid, setIsEditing]);

  const discardAndExit = useCallback(() => {
    dispatchDraft({ type: 'reset' });
    setSaveErrors([]);
    setShowExitConfirm(false);
    setIsEditing(false);
    pendingExitActionRef.current?.();
    pendingExitActionRef.current = null;
  }, [setIsEditing]);

  const requestExit = useCallback((afterExit?: () => void) => {
    pendingExitActionRef.current = afterExit ?? null;
    if (dirty) {
      setShowExitConfirm(true);
    } else {
      discardAndExit();
    }
  }, [dirty, discardAndExit]);

  const handleVariantSelect = useCallback(
    (variantId: string) => {
      setSelectedVariantId(variantId);
      if (isEditing && prompt) {
        dispatchDraft({
          type: 'seed',
          prompt,
          activeVariantId: variantId,
        });
      }
    },
    [isEditing, prompt],
  );

  useEffect(() => {
    function handleExitRequest(event: Event) {
      requestExit((event as CustomEvent<PromptEditExitDetail>).detail.afterExit);
    }
    window.addEventListener(PROMPT_EDIT_EXIT_EVENT, handleExitRequest);
    return () => window.removeEventListener(PROMPT_EDIT_EXIT_EVENT, handleExitRequest);
  }, [requestExit]);

  // Cmd+S to save, Escape to cancel (only when editing)
  useEffect(() => {
    if (!isEditing) return;

    function onKeyDown(e: KeyboardEvent) {
      const meta = e.metaKey || e.ctrlKey;

      if (meta && e.key.toLowerCase() === 's') {
        e.preventDefault();
        handleSave();
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        requestExit();
        return;
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isEditing, handleSave, requestExit]);

  if (loading) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: '13px' }}
      >
        Loading...
      </div>
    );
  }

  if (!prompt) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: '13px' }}
      >
        Prompt not found
      </div>
    );
  }

  const charCount = isEditing
    ? activeVariantDraft?.content.length ?? selectedVariant?.content.length ?? 0
    : selectedVariant?.content.length ?? 0;

  return (
    <div className="flex flex-col h-full">
      {/* Sticky header */}
      <div
        className="flex-shrink-0"
        style={{
          padding: '16px 20px 12px',
          borderBottom: '1px solid var(--border)',
          background: 'var(--bg-secondary)',
        }}
      >
        {/* Title row */}
        <div className="flex items-center gap-3">
          {isEditing ? (
            <input
              ref={titleInputRef}
              type="text"
              value={drafts.metadata?.title ?? prompt.title}
              onChange={(e) =>
                dispatchDraft({ type: 'edit_metadata', title: e.target.value })
              }
              className="flex-1 min-w-0"
              style={{
                fontSize: '16px',
                fontWeight: 600,
                color: 'var(--text-primary)',
                background: 'transparent',
                border: 'none',
                borderBottom: '1px solid var(--accent)',
                outline: 'none',
                padding: '2px 0',
                margin: 0,
              }}
            />
          ) : (
            <h2
              className="flex-1 min-w-0 truncate"
              style={{
                fontSize: '16px',
                fontWeight: 600,
                color: 'var(--text-primary)',
                margin: 0,
              }}
            >
              {prompt.title}
            </h2>
          )}

          {isEditing ? (
            <>
              <button
                onClick={() => requestExit()}
                className="flex items-center justify-center rounded cursor-default"
                style={{
                  padding: '6px 12px',
                  fontSize: '12px',
                  fontWeight: 500,
                  border: '1px solid var(--border)',
                  background: 'transparent',
                  color: 'var(--text-secondary)',
                  borderRadius: 6,
                }}
              >
                Cancel
                <kbd
                  style={{
                    fontSize: '10px',
                    opacity: 0.5,
                    fontFamily: 'inherit',
                    marginLeft: 6,
                  }}
                >
                  Esc
                </kbd>
              </button>
              <button
                onClick={handleSave}
                disabled={saving || !draftsAreValid}
                className="flex items-center gap-1.5 rounded cursor-default outline-none"
                style={{
                  padding: '6px 16px',
                  fontSize: '12px',
                  fontWeight: 500,
                  border: 'none',
                  background:
                    saving || !draftsAreValid
                      ? 'color-mix(in srgb, var(--accent) 40%, transparent)'
                      : 'var(--accent)',
                  color: '#ffffff',
                  borderRadius: 6,
                  transition: 'background 0.15s ease',
                }}
              >
                {saving ? 'Saving...' : 'Save'}
                <kbd
                  style={{
                    fontSize: '10px',
                    opacity: 0.7,
                    fontFamily: 'inherit',
                  }}
                >
                  {'\u2318'}S
                </kbd>
              </button>
            </>
          ) : (
            <>
              {selectedVariant && (
                <CopyButton
                  content={selectedVariant.content}
                  promptId={prompt.id}
                  variantId={selectedVariant.id}
                />
              )}
              <button
                onClick={() => setIsEditing(true)}
                className="flex items-center justify-center rounded cursor-default"
                style={{
                  padding: '6px 12px',
                  fontSize: '12px',
                  fontWeight: 500,
                  border: '1px solid var(--border)',
                  background: 'transparent',
                  color: 'var(--text-secondary)',
                  borderRadius: 6,
                }}
              >
                Edit
                <kbd
                  style={{
                    fontSize: '10px',
                    opacity: 0.5,
                    fontFamily: 'inherit',
                    marginLeft: 6,
                  }}
                >
                  {'\u2318'}E
                </kbd>
              </button>
            </>
          )}
        </div>

        {isEditing && (
          <input
            type="text"
            aria-label="Prompt description"
            value={drafts.metadata?.description ?? prompt.description ?? ''}
            onChange={(event) =>
              dispatchDraft({
                type: 'edit_metadata',
                description: event.target.value,
              })
            }
            placeholder="Description"
            style={{
              width: '100%',
              marginTop: 10,
              padding: '4px 0',
              fontSize: '12px',
              color: 'var(--text-secondary)',
              background: 'transparent',
              border: 'none',
              borderBottom: '1px solid var(--border)',
              outline: 'none',
            }}
          />
        )}

        {saveErrors.length > 0 && (
          <div
            role="alert"
            style={{
              marginTop: 10,
              fontSize: '11px',
              lineHeight: 1.5,
              color: '#ff453a',
            }}
          >
            {saveErrors.map((error) => (
              <div key={error}>{error}</div>
            ))}
          </div>
        )}

        {/* Tags and copy stats (visible in view mode only) */}
        {!isEditing && (
          <div className="flex items-center gap-3 mt-2 flex-wrap">
            <TagPills tags={prompt.tags} promptId={prompt.id} />
            <span
              style={{
                fontSize: '11px',
                color: 'var(--text-secondary)',
                whiteSpace: 'nowrap',
              }}
            >
              Copied {prompt.copy_count} time{prompt.copy_count !== 1 ? 's' : ''}
              {prompt.last_copied_at && (
                <> &middot; Last: {formatRelativeTime(prompt.last_copied_at)}</>
              )}
            </span>
          </div>
        )}
      </div>

      {/* Variant selector */}
      <VariantSelector
        variants={prompt.variants}
        selectedId={selectedVariantId ?? ''}
        onSelect={handleVariantSelect}
      />

      {/* Content area */}
      <div className="flex-1 overflow-y-auto" style={{ padding: '20px' }}>
        {isEditing ? (
          <div className="flex flex-col h-full gap-3">
            <input
              type="text"
              aria-label="Variant label"
              value={activeVariantDraft?.label ?? selectedVariant?.label ?? ''}
              onChange={(event) => {
                if (!selectedVariantId) return;
                dispatchDraft({
                  type: 'edit_variant',
                  variantId: selectedVariantId,
                  label: event.target.value,
                });
              }}
              style={{
                width: '100%',
                padding: '5px 0',
                fontSize: '12px',
                fontWeight: 600,
                color: 'var(--text-primary)',
                background: 'transparent',
                border: 'none',
                borderBottom: '1px solid var(--border)',
                outline: 'none',
              }}
            />
            <textarea
              aria-label="Variant content"
              value={activeVariantDraft?.content ?? selectedVariant?.content ?? ''}
              onChange={(event) => {
                if (!selectedVariantId) return;
                dispatchDraft({
                  type: 'edit_variant',
                  variantId: selectedVariantId,
                  content: event.target.value,
                });
              }}
              style={{
                width: '100%',
                flex: 1,
                fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
                fontSize: '12px',
                lineHeight: 1.7,
                color: 'var(--text-primary)',
                background: 'transparent',
                border: 'none',
                outline: 'none',
                resize: 'none',
                padding: 0,
                margin: 0,
              }}
            />
          </div>
        ) : (
          <pre
            style={{
              fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
              fontSize: '12px',
              lineHeight: 1.7,
              whiteSpace: 'pre-wrap',
              wordBreak: 'break-word',
              color: 'var(--text-primary)',
              margin: 0,
            }}
          >
            {highlightedContent}
          </pre>
        )}
      </div>

      {/* Footer */}
      <div
        className="flex items-center gap-4 flex-shrink-0"
        style={{
          padding: '10px 20px',
          borderTop: '1px solid var(--border)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
        }}
      >
        <span>Created {formatDate(prompt.created_at)}</span>
        <span>Modified {formatDate(prompt.updated_at)}</span>
        <span>{charCount.toLocaleString()} characters</span>
        {isEditing && (
          <span style={{ color: 'var(--accent)', fontWeight: 500 }}>
            {dirty ? 'Unsaved changes' : 'Editing'}
          </span>
        )}
      </div>

      {showExitConfirm && (
        <div
          role="dialog"
          aria-modal="true"
          aria-labelledby="unsaved-edits-title"
          style={{
            position: 'fixed',
            inset: 0,
            zIndex: 9500,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            background: 'rgba(0, 0, 0, 0.45)',
          }}
        >
          <div
            style={{
              width: 420,
              maxWidth: 'calc(100vw - 32px)',
              padding: 20,
              borderRadius: 8,
              background: 'var(--bg-secondary)',
              border: '1px solid var(--border)',
              boxShadow: '0 20px 60px rgba(0, 0, 0, 0.35)',
            }}
          >
            <h3
              id="unsaved-edits-title"
              style={{ margin: 0, fontSize: '15px', color: 'var(--text-primary)' }}
            >
              Unsaved edits
            </h3>
            <p
              style={{
                margin: '8px 0 18px',
                fontSize: '12px',
                lineHeight: 1.5,
                color: 'var(--text-secondary)',
              }}
            >
              {describeDirtyDrafts(drafts)}
            </p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => {
                  pendingExitActionRef.current = null;
                  setShowExitConfirm(false);
                }}
                style={confirmSecondaryButtonStyle}
              >
                Cancel
              </button>
              <button onClick={discardAndExit} style={confirmSecondaryButtonStyle}>
                Discard
              </button>
              <button
                onClick={() => void handleSave()}
                disabled={saving || !draftsAreValid}
                style={{
                  ...confirmPrimaryButtonStyle,
                  opacity: saving || !draftsAreValid ? 0.5 : 1,
                }}
              >
                {saving ? 'Saving...' : 'Save All'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

const confirmSecondaryButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  fontSize: '12px',
  borderRadius: 6,
  border: '1px solid var(--border)',
  background: 'transparent',
  color: 'var(--text-secondary)',
};

const confirmPrimaryButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  fontSize: '12px',
  fontWeight: 600,
  borderRadius: 6,
  border: 'none',
  background: 'var(--accent)',
  color: '#ffffff',
};
