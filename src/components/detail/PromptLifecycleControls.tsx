import { useEffect, useMemo, useState } from 'react';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';
import { useCollections } from '../../lib/hooks';
import type { PromptUsage, PromptWithVariants } from '../../lib/types';
import { TagPills } from './TagPills';
import { Modal } from '../shared/Modal';

interface Props {
  prompt: PromptWithVariants;
  selectedVariantId: string | null;
  isEditing: boolean;
  hasUnsavedDrafts: boolean;
  onSelectVariant: (variantId: string) => void;
}

export function formatPromptUsage(usage: PromptUsage): string {
  if (usage.step_count === 0) {
    return 'This prompt is not used in any playbooks.';
  }
  const titles =
    usage.playbook_titles.length > 0 ? `: ${usage.playbook_titles.join(', ')}` : '';
  return `Used in ${usage.step_count} step(s) across ${usage.playbook_count} playbook(s)${titles}.`;
}

export function PromptLifecycleControls({
  prompt,
  selectedVariantId,
  isEditing,
  hasUnsavedDrafts,
  onSelectVariant,
}: Props) {
  const {
    activeCollectionId,
    refreshCounter,
    setSelectedPromptId,
  } = useAppContext();
  const { collections } = useCollections(refreshCounter);
  const manualCollections = useMemo(
    () => collections.filter((collection) => !collection.is_smart),
    [collections],
  );
  const activeManualCollection = manualCollections.find(
    (collection) => collection.id === activeCollectionId,
  );
  const [collectionId, setCollectionId] = useState('');
  const [tagInput, setTagInput] = useState('');
  const [showVariantForm, setShowVariantForm] = useState(false);
  const [variantLabel, setVariantLabel] = useState('');
  const [variantContent, setVariantContent] = useState('');
  const [pendingVariantDelete, setPendingVariantDelete] = useState(false);
  const [deleteUsage, setDeleteUsage] = useState<PromptUsage | null>(null);
  const [showPromptDelete, setShowPromptDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (
      collectionId &&
      manualCollections.some((collection) => collection.id === collectionId)
    ) {
      return;
    }
    setCollectionId(manualCollections[0]?.id ?? '');
  }, [collectionId, manualCollections]);

  useEffect(() => {
    setError(null);
    setShowVariantForm(false);
    setPendingVariantDelete(false);
    setShowPromptDelete(false);
    setDeleteUsage(null);
  }, [prompt.id]);

  const selectedVariant =
    prompt.variants.find((variant) => variant.id === selectedVariantId) ?? null;

  async function run(operation: () => Promise<unknown>) {
    setBusy(true);
    setError(null);
    try {
      await operation();
    } catch (operationError) {
      setError(String(operationError));
    } finally {
      setBusy(false);
    }
  }

  async function addTags() {
    const names = tagInput
      .split(',')
      .map((name) => name.trim())
      .filter(Boolean);
    if (names.length === 0) return;
    await run(async () => {
      await api.tags.addToPrompt(prompt.id, names);
      setTagInput('');
    });
  }

  async function addVariant() {
    if (!variantLabel.trim() || !variantContent.trim()) return;
    await run(async () => {
      const created = await api.variants.add(
        prompt.id,
        variantLabel.trim(),
        variantContent,
      );
      setVariantLabel('');
      setVariantContent('');
      setShowVariantForm(false);
      onSelectVariant(created.id);
    });
  }

  async function deleteVariant() {
    if (!selectedVariant || prompt.variants.length <= 1 || hasUnsavedDrafts) return;
    const replacement = prompt.variants.find(
      (variant) => variant.id !== selectedVariant.id,
    );
    await run(async () => {
      await api.variants.delete(selectedVariant.id);
      setPendingVariantDelete(false);
      if (replacement) onSelectVariant(replacement.id);
    });
  }

  async function openPromptDelete() {
    await run(async () => {
      const usage = await api.prompts.usage(prompt.id);
      setDeleteUsage(usage);
      setShowPromptDelete(true);
    });
  }

  async function deletePrompt() {
    await run(async () => {
      await api.prompts.delete(prompt.id);
      setShowPromptDelete(false);
      setSelectedPromptId(null);
    });
  }

  return (
    <>
      <div
        className="flex flex-wrap items-center gap-2 flex-shrink-0"
        style={{
          minHeight: 42,
          padding: '7px 16px',
          borderBottom: '1px solid var(--border)',
          background: 'color-mix(in srgb, var(--bg-secondary) 92%, var(--bg-primary))',
        }}
      >
        {isEditing ? (
          <>
            <button
              type="button"
              onClick={() => setShowVariantForm((visible) => !visible)}
              style={secondaryButtonStyle}
            >
              Add variant
            </button>
            <button
              type="button"
              disabled={
                busy ||
                hasUnsavedDrafts ||
                !selectedVariant ||
                prompt.variants.length <= 1
              }
              title={
                hasUnsavedDrafts
                  ? 'Save or discard edits before deleting a variant'
                  : 'Delete selected variant'
              }
              onClick={() => setPendingVariantDelete(true)}
              style={{
                ...secondaryButtonStyle,
                color: '#ff453a',
                opacity:
                  hasUnsavedDrafts || prompt.variants.length <= 1 ? 0.45 : 1,
              }}
            >
              Delete variant
            </button>
            <div
              className="flex items-center gap-1.5 flex-wrap"
              style={{ marginLeft: 6 }}
            >
              <TagPills
                tags={prompt.tags}
                onRemove={(tagId) =>
                  void run(() => api.tags.removeFromPrompt(prompt.id, tagId))
                }
              />
              <input
                aria-label="Add tags"
                value={tagInput}
                onChange={(event) => setTagInput(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    void addTags();
                  }
                }}
                placeholder="Add tags"
                style={compactInputStyle}
              />
              <button
                type="button"
                disabled={busy || !tagInput.trim()}
                onClick={() => void addTags()}
                style={secondaryButtonStyle}
              >
                Add
              </button>
            </div>
          </>
        ) : (
          <>
            {manualCollections.length > 0 && (
              <>
                <select
                  aria-label="Collection"
                  value={collectionId}
                  onChange={(event) => setCollectionId(event.target.value)}
                  style={{ ...compactInputStyle, width: 150 }}
                >
                  {manualCollections.map((collection) => (
                    <option key={collection.id} value={collection.id}>
                      {collection.name}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  disabled={busy || !collectionId}
                  onClick={() =>
                    void run(() =>
                      api.collections.addPrompt(collectionId, prompt.id),
                    )
                  }
                  style={secondaryButtonStyle}
                >
                  Add to collection
                </button>
              </>
            )}
            {activeManualCollection && (
              <button
                type="button"
                disabled={busy}
                onClick={() =>
                  void run(() =>
                    api.collections.removePrompt(
                      activeManualCollection.id,
                      prompt.id,
                    ),
                  )
                }
                style={secondaryButtonStyle}
              >
                Remove from {activeManualCollection.name}
              </button>
            )}
            <button
              type="button"
              aria-label="Delete prompt"
              title="Delete prompt"
              disabled={busy}
              onClick={() => void openPromptDelete()}
              style={{ ...iconButtonStyle, marginLeft: 'auto', color: '#ff453a' }}
            >
              <TrashIcon />
            </button>
          </>
        )}
        {error && (
          <span role="alert" style={{ fontSize: 11, color: '#ff453a' }}>
            {error}
          </span>
        )}
      </div>

      {isEditing && showVariantForm && (
        <div
          className="grid gap-2 flex-shrink-0"
          style={{
            gridTemplateColumns: 'minmax(120px, 0.35fr) minmax(200px, 1fr) auto',
            padding: '8px 16px',
            borderBottom: '1px solid var(--border)',
          }}
        >
          <input
            aria-label="New variant label"
            value={variantLabel}
            onChange={(event) => setVariantLabel(event.target.value)}
            placeholder="Variant label"
            style={compactInputStyle}
          />
          <input
            aria-label="New variant content"
            value={variantContent}
            onChange={(event) => setVariantContent(event.target.value)}
            placeholder="Variant content"
            style={compactInputStyle}
          />
          <button
            type="button"
            disabled={busy || !variantLabel.trim() || !variantContent.trim()}
            onClick={() => void addVariant()}
            style={primaryButtonStyle}
          >
            Create
          </button>
        </div>
      )}

      {pendingVariantDelete && selectedVariant && (
        <ConfirmDialog
          id={`delete-variant-${selectedVariant.id}`}
          title="Delete variant?"
          description={`Delete "${selectedVariant.label}"? This cannot be undone.`}
          confirmLabel="Delete variant"
          busy={busy}
          onCancel={() => setPendingVariantDelete(false)}
          onConfirm={() => void deleteVariant()}
        />
      )}

      {showPromptDelete && deleteUsage && (
        <ConfirmDialog
          id={`delete-prompt-${prompt.id}`}
          title={`Delete "${prompt.title}"?`}
          description={`${formatPromptUsage(deleteUsage)} The prompt will appear as removed in affected playbooks.`}
          confirmLabel="Delete prompt"
          busy={busy}
          onCancel={() => setShowPromptDelete(false)}
          onConfirm={() => void deletePrompt()}
        />
      )}
    </>
  );
}

function ConfirmDialog({
  id,
  title,
  description,
  confirmLabel,
  busy,
  onCancel,
  onConfirm,
}: {
  id: string;
  title: string;
  description: string;
  confirmLabel: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <Modal
      id={id}
      isOpen
      onClose={onCancel}
      ariaLabel={title}
      panelStyle={{ padding: 20 }}
    >
      <h3 style={{ margin: 0, fontSize: 15 }}>{title}</h3>
      <p
        style={{
          margin: '8px 0 18px',
          color: 'var(--text-secondary)',
          fontSize: 12,
          lineHeight: 1.5,
        }}
      >
        {description}
      </p>
      <div className="flex justify-end gap-2">
        <button type="button" onClick={onCancel} style={secondaryButtonStyle}>
          Cancel
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={onConfirm}
          style={{ ...primaryButtonStyle, background: '#ff453a' }}
        >
          {busy ? 'Deleting...' : confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

function TrashIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
    >
      <path d="M3 4h10M6 4V2h4v2M5 6v6M8 6v6M11 6v6M4 4l.6 10h6.8L12 4" />
    </svg>
  );
}

const compactInputStyle: React.CSSProperties = {
  minHeight: 28,
  border: '1px solid var(--border)',
  borderRadius: 5,
  padding: '4px 7px',
  background: 'var(--bg-primary)',
  color: 'var(--text-primary)',
  fontSize: 11,
};

const secondaryButtonStyle: React.CSSProperties = {
  minHeight: 28,
  padding: '4px 9px',
  border: '1px solid var(--border)',
  borderRadius: 5,
  background: 'transparent',
  color: 'var(--text-secondary)',
  fontSize: 11,
};

const primaryButtonStyle: React.CSSProperties = {
  minHeight: 28,
  padding: '4px 11px',
  border: 0,
  borderRadius: 5,
  background: 'var(--accent)',
  color: '#ffffff',
  fontSize: 11,
  fontWeight: 600,
};

const iconButtonStyle: React.CSSProperties = {
  width: 28,
  height: 28,
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: '1px solid var(--border)',
  borderRadius: 5,
  background: 'transparent',
};
