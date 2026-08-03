import { useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';
import {
  applyPlaybookDraftProgress,
  createDraftStep,
  createPlaybookDraft,
  persistPlaybookDraft,
  PlaybookDraftPersistenceError,
  validatePlaybookDraft,
  type PlaybookDraft,
  type PlaybookDraftStep,
} from '../../lib/playbookDraft';
import type {
  PlaybookStepWithPrompt,
  PlaybookWithSteps,
  PromptListItem,
} from '../../lib/types';

interface Props {
  prompts: PromptListItem[];
}

const controlStyle: React.CSSProperties = {
  width: '100%',
  border: '1px solid var(--border)',
  borderRadius: 6,
  background: 'var(--bg-primary)',
  color: 'var(--text-primary)',
  fontSize: 12,
  padding: '8px 10px',
  outline: 'none',
};

export function PlaybookBuilder({ prompts }: Props) {
  const {
    activePlaybookId,
    playbookBuilderMode,
    setPlaybookBuilderMode,
    setActivePlaybookId,
    showToast,
  } = useAppContext();
  const [draft, setDraft] = useState<PlaybookDraft>(() => createPlaybookDraft());
  const [loading, setLoading] = useState(playbookBuilderMode === 'edit');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionConflict, setSessionConflict] = useState(false);
  const [missingByStep, setMissingByStep] = useState<Map<string, Set<string>>>(
    new Map(),
  );
  const [knownTitles, setKnownTitles] = useState<Map<string, string>>(new Map());
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    setSessionConflict(false);

    if (playbookBuilderMode === 'create') {
      setDraft(createPlaybookDraft());
      setMissingByStep(new Map());
      setKnownTitles(new Map());
      setLoading(false);
      requestAnimationFrame(() => titleRef.current?.focus());
      return () => {
        cancelled = true;
      };
    }

    if (playbookBuilderMode !== 'edit' || !activePlaybookId) {
      setLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setLoading(true);
    api.playbooks
      .get(activePlaybookId)
      .then((playbook) => {
        if (cancelled) return;
        setDraft(createPlaybookDraft(playbook));
        setMissingByStep(findMissingReferences(playbook));
        setKnownTitles(collectKnownTitles(playbook));
      })
      .catch((fetchError) => {
        if (!cancelled) setError(String(fetchError));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [activePlaybookId, playbookBuilderMode]);

  const promptTitles = useMemo(() => {
    const titles = new Map(knownTitles);
    prompts.forEach((prompt) => titles.set(prompt.id, prompt.title));
    return titles;
  }, [knownTitles, prompts]);

  const validationErrors = useMemo(() => {
    const errors = validatePlaybookDraft(draft);
    draft.steps.forEach((step, index) => {
      const missing = missingByStep.get(step.key);
      if (missing && missing.size > 0) {
        errors.push(`Step ${index + 1} contains a removed prompt`);
      }
    });
    return errors;
  }, [draft, missingByStep]);

  function updateDraftStep(
    key: string,
    update: (step: PlaybookDraftStep) => PlaybookDraftStep,
  ) {
    setDraft((current) => ({
      ...current,
      steps: current.steps.map((step) => (step.key === key ? update(step) : step)),
    }));
  }

  function setStepType(key: string, stepType: 'single' | 'choice') {
    updateDraftStep(key, (step) => {
      if (step.step_type === stepType) return step;
      if (stepType === 'choice') {
        return {
          ...step,
          step_type: 'choice',
          choice_prompt_ids: step.prompt_id ? [step.prompt_id] : [],
          prompt_id: null,
        };
      }
      return {
        ...step,
        step_type: 'single',
        prompt_id: step.choice_prompt_ids[0] ?? null,
        choice_prompt_ids: [],
      };
    });
    setMissingByStep((current) => {
      const next = new Map(current);
      next.delete(key);
      return next;
    });
  }

  function selectSinglePrompt(key: string, promptId: string) {
    updateDraftStep(key, (step) => ({ ...step, prompt_id: promptId }));
    clearMissingReference(key, promptId);
  }

  function toggleChoicePrompt(key: string, promptId: string) {
    updateDraftStep(key, (step) => ({
      ...step,
      choice_prompt_ids: step.choice_prompt_ids.includes(promptId)
        ? step.choice_prompt_ids.filter((id) => id !== promptId)
        : [...step.choice_prompt_ids, promptId],
    }));
    clearMissingReference(key, promptId);
  }

  function clearMissingReference(key: string, promptId: string) {
    setMissingByStep((current) => {
      const missing = current.get(key);
      if (!missing?.has(promptId)) return current;
      const next = new Map(current);
      const remaining = new Set(missing);
      remaining.delete(promptId);
      if (remaining.size === 0) next.delete(key);
      else next.set(key, remaining);
      return next;
    });
  }

  function removeMissingReference(key: string, promptId: string) {
    updateDraftStep(key, (step) => ({
      ...step,
      prompt_id: step.prompt_id === promptId ? null : step.prompt_id,
      choice_prompt_ids: step.choice_prompt_ids.filter((id) => id !== promptId),
    }));
    clearMissingReference(key, promptId);
  }

  function moveStep(index: number, direction: -1 | 1) {
    setDraft((current) => {
      const target = index + direction;
      if (target < 0 || target >= current.steps.length) return current;
      const steps = [...current.steps];
      [steps[index], steps[target]] = [steps[target], steps[index]];
      return { ...current, steps };
    });
  }

  async function save() {
    if (validationErrors.length > 0 || saving) return;
    setSaving(true);
    setError(null);
    setSessionConflict(false);
    try {
      const playbookId = await persistPlaybookDraft(draft, {
        createPlaybook: api.playbooks.create,
        updatePlaybook: api.playbooks.update,
        addStep: api.playbooks.addStep,
        updateStep: api.playbooks.updateStep,
        removeStep: api.playbooks.removeStep,
        reorderSteps: api.playbooks.reorderSteps,
      });
      setActivePlaybookId(playbookId);
      setPlaybookBuilderMode(null);
    } catch (saveError) {
      if (saveError instanceof PlaybookDraftPersistenceError) {
        setDraft((current) =>
          applyPlaybookDraftProgress(current, saveError.progress),
        );
      }
      const message = String(saveError);
      setError(message);
      showToast(`Couldn't save playbook: ${message}`, 'error');
      setSessionConflict(
        message.includes('End the active session to edit this playbook'),
      );
    } finally {
      setSaving(false);
    }
  }

  async function endSessionAndSave() {
    setSaving(true);
    setError(null);
    try {
      await api.session.end();
      setSessionConflict(false);
    } catch (endError) {
      const message = String(endError);
      setError(message);
      showToast(`Couldn't end session: ${message}`, 'error');
      setSaving(false);
      return;
    }
    setSaving(false);
    await save();
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ color: 'var(--text-secondary)', fontSize: 13 }}>
        Loading playbook...
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      <div
        className="flex items-center gap-3 flex-shrink-0"
        style={{
          padding: '14px 20px',
          borderBottom: '1px solid var(--border)',
          background: 'var(--bg-secondary)',
        }}
      >
        <h2 className="flex-1" style={{ margin: 0, fontSize: 16, fontWeight: 650 }}>
          {playbookBuilderMode === 'edit' ? 'Edit Playbook' : 'New Playbook'}
        </h2>
        <button
          type="button"
          onClick={() => setPlaybookBuilderMode(null)}
          style={secondaryButtonStyle}
        >
          Cancel
        </button>
        <button
          type="button"
          onClick={() => void save()}
          disabled={saving || validationErrors.length > 0}
          style={{
            ...primaryButtonStyle,
            opacity: saving || validationErrors.length > 0 ? 0.5 : 1,
          }}
        >
          {saving ? 'Saving...' : 'Save Playbook'}
        </button>
      </div>

      <div className="flex-1 overflow-y-auto" style={{ padding: '18px 20px 28px' }}>
        <div className="grid gap-3" style={{ gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)' }}>
          <label style={labelStyle}>
            Name
            <input
              ref={titleRef}
              aria-label="Playbook name"
              value={draft.title}
              onChange={(event) =>
                setDraft((current) => ({ ...current, title: event.target.value }))
              }
              style={controlStyle}
            />
          </label>
          <label style={labelStyle}>
            Description
            <input
              aria-label="Playbook description"
              value={draft.description}
              onChange={(event) =>
                setDraft((current) => ({
                  ...current,
                  description: event.target.value,
                }))
              }
              style={controlStyle}
            />
          </label>
        </div>

        {(error || validationErrors.length > 0) && (
          <div
            role="alert"
            style={{
              marginTop: 12,
              padding: '9px 10px',
              borderRadius: 6,
              color: '#ff453a',
              background: 'color-mix(in srgb, #ff453a 8%, transparent)',
              fontSize: 11,
              lineHeight: 1.5,
            }}
          >
            {error ?? validationErrors[0]}
            {sessionConflict && (
              <button
                type="button"
                onClick={() => void endSessionAndSave()}
                style={{ ...secondaryButtonStyle, marginLeft: 10, color: '#ff453a' }}
              >
                End session and edit
              </button>
            )}
          </div>
        )}

        <div className="flex items-center justify-between" style={{ marginTop: 20, marginBottom: 8 }}>
          <h3 style={{ margin: 0, fontSize: 13, fontWeight: 650 }}>Steps</h3>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() =>
                setDraft((current) => ({
                  ...current,
                  steps: [...current.steps, createDraftStep('single')],
                }))
              }
              style={secondaryButtonStyle}
            >
              + Single
            </button>
            <button
              type="button"
              onClick={() =>
                setDraft((current) => ({
                  ...current,
                  steps: [...current.steps, createDraftStep('choice')],
                }))
              }
              style={secondaryButtonStyle}
            >
              + Choice
            </button>
          </div>
        </div>

        {draft.steps.length === 0 ? (
          <div style={{ padding: '24px 0', color: 'var(--text-secondary)', fontSize: 12 }}>
            No steps
          </div>
        ) : (
          <div className="flex flex-col gap-8">
            {draft.steps.map((step, index) => (
              <div
                key={step.key}
                style={{
                  borderTop: '1px solid var(--border)',
                  paddingTop: 12,
                }}
              >
                <div className="flex items-center gap-2">
                  <strong style={{ fontSize: 12 }}>Step {index + 1}</strong>
                  <div
                    className="inline-flex"
                    style={{
                      padding: 2,
                      borderRadius: 6,
                      background: 'color-mix(in srgb, var(--text-secondary) 10%, transparent)',
                    }}
                  >
                    {(['single', 'choice'] as const).map((type) => (
                      <button
                        key={type}
                        type="button"
                        onClick={() => setStepType(step.key, type)}
                        style={{
                          border: 'none',
                          borderRadius: 4,
                          padding: '4px 9px',
                          fontSize: 11,
                          background:
                            step.step_type === type ? 'var(--bg-primary)' : 'transparent',
                          color:
                            step.step_type === type
                              ? 'var(--text-primary)'
                              : 'var(--text-secondary)',
                        }}
                      >
                        {type === 'single' ? 'Single' : 'Choice'}
                      </button>
                    ))}
                  </div>
                  <div className="flex gap-1" style={{ marginLeft: 'auto' }}>
                    <button
                      type="button"
                      aria-label={`Move step ${index + 1} up`}
                      title="Move up"
                      disabled={index === 0}
                      onClick={() => moveStep(index, -1)}
                      style={iconButtonStyle}
                    >
                      {'\u2191'}
                    </button>
                    <button
                      type="button"
                      aria-label={`Move step ${index + 1} down`}
                      title="Move down"
                      disabled={index === draft.steps.length - 1}
                      onClick={() => moveStep(index, 1)}
                      style={iconButtonStyle}
                    >
                      {'\u2193'}
                    </button>
                    <button
                      type="button"
                      aria-label={`Remove step ${index + 1}`}
                      title="Remove step"
                      onClick={() =>
                        setDraft((current) => ({
                          ...current,
                          steps: current.steps.filter(
                            (candidate) => candidate.key !== step.key,
                          ),
                        }))
                      }
                      style={{ ...iconButtonStyle, color: '#ff453a' }}
                    >
                      {'\u00d7'}
                    </button>
                  </div>
                </div>

                <StepPromptPicker
                  step={step}
                  prompts={prompts}
                  promptTitles={promptTitles}
                  missingIds={missingByStep.get(step.key) ?? new Set()}
                  onSingleSelect={(id) => selectSinglePrompt(step.key, id)}
                  onChoiceToggle={(id) => toggleChoicePrompt(step.key, id)}
                  onRemoveMissing={(id) => removeMissingReference(step.key, id)}
                />

                <label style={{ ...labelStyle, marginTop: 10 }}>
                  Instructions
                  <textarea
                    aria-label={`Step ${index + 1} instructions`}
                    value={step.instructions}
                    onChange={(event) =>
                      updateDraftStep(step.key, (current) => ({
                        ...current,
                        instructions: event.target.value,
                      }))
                    }
                    rows={2}
                    style={{ ...controlStyle, resize: 'vertical' }}
                  />
                </label>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function StepPromptPicker({
  step,
  prompts,
  promptTitles,
  missingIds,
  onSingleSelect,
  onChoiceToggle,
  onRemoveMissing,
}: {
  step: PlaybookDraftStep;
  prompts: PromptListItem[];
  promptTitles: Map<string, string>;
  missingIds: Set<string>;
  onSingleSelect: (id: string) => void;
  onChoiceToggle: (id: string) => void;
  onRemoveMissing: (id: string) => void;
}) {
  const [query, setQuery] = useState('');
  const normalized = query.trim().toLocaleLowerCase();
  const visiblePrompts = prompts
    .filter((prompt) => {
      if (!normalized) return true;
      return (
        prompt.title.toLocaleLowerCase().includes(normalized) ||
        prompt.tags.some((tag) => tag.name.toLocaleLowerCase().includes(normalized))
      );
    })
    .slice(0, 20);
  const selectedIds =
    step.step_type === 'single'
      ? step.prompt_id
        ? [step.prompt_id]
        : []
      : step.choice_prompt_ids;

  return (
    <div style={{ marginTop: 10 }}>
      {selectedIds.length > 0 && (
        <div className="flex flex-wrap gap-1.5" style={{ marginBottom: 7 }}>
          {selectedIds.map((id) => (
            <span
              key={id}
              className="inline-flex items-center gap-1"
              style={{
                padding: '3px 7px',
                borderRadius: 4,
                fontSize: 11,
                color: missingIds.has(id) ? '#ff453a' : 'var(--text-primary)',
                background: missingIds.has(id)
                  ? 'color-mix(in srgb, #ff453a 10%, transparent)'
                  : 'color-mix(in srgb, var(--accent) 10%, transparent)',
              }}
            >
              {missingIds.has(id) ? 'Prompt removed' : promptTitles.get(id) ?? id}
              {missingIds.has(id) && (
                <button
                  type="button"
                  aria-label="Remove missing prompt"
                  onClick={() => onRemoveMissing(id)}
                  style={{
                    border: 0,
                    padding: 0,
                    background: 'transparent',
                    color: '#ff453a',
                    fontSize: 14,
                  }}
                >
                  {'\u00d7'}
                </button>
              )}
            </span>
          ))}
        </div>
      )}
      <input
        aria-label="Search prompts for step"
        placeholder="Search prompts"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        style={controlStyle}
      />
      <div
        style={{
          maxHeight: 136,
          overflowY: 'auto',
          border: '1px solid var(--border)',
          borderTop: 0,
          background: 'var(--bg-primary)',
        }}
      >
        {visiblePrompts.map((prompt) => {
          const selected = selectedIds.includes(prompt.id);
          return (
            <button
              key={prompt.id}
              type="button"
              onClick={() =>
                step.step_type === 'single'
                  ? onSingleSelect(prompt.id)
                  : onChoiceToggle(prompt.id)
              }
              className="flex items-center gap-2 w-full text-left"
              style={{
                minHeight: 32,
                padding: '6px 9px',
                border: 0,
                borderBottom: '1px solid var(--border)',
                background: selected
                  ? 'color-mix(in srgb, var(--accent) 10%, transparent)'
                  : 'transparent',
                color: 'var(--text-primary)',
                fontSize: 11,
              }}
            >
              {step.step_type === 'choice' && (
                <input type="checkbox" checked={selected} readOnly tabIndex={-1} />
              )}
              <span className="truncate">{prompt.title}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function findMissingReferences(playbook: PlaybookWithSteps): Map<string, Set<string>> {
  const missing = new Map<string, Set<string>>();
  playbook.steps.forEach((step) => {
    if (step.step_type === 'single' && step.prompt_id && !step.prompt) {
      missing.set(step.id, new Set([step.prompt_id]));
    }
    if (step.step_type === 'choice') {
      const resolved = new Set(step.choice_prompts.map((prompt) => prompt.id));
      const missingChoices = step.choice_prompt_ids.filter((id) => !resolved.has(id));
      if (missingChoices.length > 0) missing.set(step.id, new Set(missingChoices));
    }
  });
  return missing;
}

function collectKnownTitles(playbook: PlaybookWithSteps): Map<string, string> {
  const titles = new Map<string, string>();
  playbook.steps.forEach((step: PlaybookStepWithPrompt) => {
    if (step.prompt) titles.set(step.prompt.id, step.prompt.title);
    step.choice_prompts.forEach((prompt) => titles.set(prompt.id, prompt.title));
  });
  return titles;
}

const labelStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 5,
  color: 'var(--text-secondary)',
  fontSize: 10,
  fontWeight: 600,
};

const secondaryButtonStyle: React.CSSProperties = {
  padding: '6px 10px',
  border: '1px solid var(--border)',
  borderRadius: 6,
  background: 'transparent',
  color: 'var(--text-secondary)',
  fontSize: 11,
};

const primaryButtonStyle: React.CSSProperties = {
  padding: '7px 12px',
  border: 0,
  borderRadius: 6,
  background: 'var(--accent)',
  color: '#ffffff',
  fontSize: 11,
  fontWeight: 600,
};

const iconButtonStyle: React.CSSProperties = {
  width: 28,
  height: 28,
  border: '1px solid var(--border)',
  borderRadius: 5,
  background: 'transparent',
  color: 'var(--text-secondary)',
  fontSize: 14,
};
