import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { api } from '../../lib/api';
import {
  canApplyCopyEffect,
  startCopy,
  type CopyOperation,
} from '../../lib/copy';
import { useAppContext } from '../../lib/context';
import { usePlaybookSession } from '../../lib/hooks';
import { getPlaybookProgress } from '../../lib/playbook';
import type { PlaybookWithSteps } from '../../lib/types';
import { PlaybookStep } from './PlaybookStep';
import type { PlaybookCopyTarget, StepStatus } from './PlaybookStep';
import { FillVariablesModal } from '../shared/FillVariablesModal';

interface Props {
  playbookId: string;
}

export function PlaybookStepper({ playbookId }: Props) {
  const [playbook, setPlaybook] = useState<PlaybookWithSteps | null>(null);
  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<Error | null>(null);
  const [activeCopy, setActiveCopy] = useState<CopyOperation | null>(null);
  const [fillCopy, setFillCopy] = useState<{
    operation: CopyOperation;
    content: string;
  } | null>(null);
  const copyRef = useRef<CopyOperation | null>(null);
  const liveRef = useRef(true);
  const {
    refreshCounter,
    triggerRefresh,
    setPlaybookBuilderMode,
    showToast,
  } = useAppContext();
  const { data: session, error: sessionError } =
    usePlaybookSession(refreshCounter);

  useEffect(() => {
    liveRef.current = true;
    return () => {
      liveRef.current = false;
      copyRef.current?.fill?.cancel();
      copyRef.current = null;
    };
  }, []);

  useEffect(() => {
    copyRef.current?.fill?.cancel();
    copyRef.current = null;
    setActiveCopy(null);
    setFillCopy(null);
  }, [playbookId]);

  // Fetch the playbook
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setFetchError(null);

    api.playbooks
      .get(playbookId)
      .then((result) => {
        if (!cancelled) setPlaybook(result);
      })
      .catch((err) => {
        if (!cancelled) setFetchError(
          err instanceof Error ? err : new Error(String(err)),
        );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [playbookId, refreshCounter]);

  const steps = useMemo(
    () => [...(playbook?.steps ?? [])].sort((a, b) => a.position - b.position),
    [playbook],
  );

  const isSessionActive = session?.active_playbook_id === playbookId;
  const currentStep = isSessionActive ? (session?.current_step ?? 0) : -1;

  const handleStartSession = useCallback(async () => {
    try {
      await api.session.start(playbookId);
    } catch (err) {
      showToast(`Couldn't start session: ${String(err)}`, 'error');
    }
  }, [playbookId, showToast]);

  const handleEndSession = useCallback(async () => {
    try {
      await api.session.end();
    } catch (err) {
      showToast(`Couldn't end session: ${String(err)}`, 'error');
    }
  }, [showToast]);

  const handleCopyAndAdvance = useCallback(
    ({ promptId, variantId, content }: PlaybookCopyTarget) => {
      if (activeCopy) return;
      const operation = startCopy({
        content,
        promptId,
        variantId,
        onAccounting: (ok, key) => {
          if (!canApplyCopyEffect(copyRef.current, key, liveRef.current)) return;
          if (!ok) {
            showToast("Copied, but couldn't record usage", 'error');
          }
        },
      });
      copyRef.current = operation;
      setActiveCopy(operation);
      if (operation.fill) setFillCopy({ operation, content });

      void operation.settled.then((result) => {
        if (!canApplyCopyEffect(copyRef.current, operation.key, liveRef.current)) {
          return;
        }
        setActiveCopy(null);
        setFillCopy(null);
        if (result.kind === 'copied' && isSessionActive) {
          void api.session.advance().catch((error) => {
            if (canApplyCopyEffect(copyRef.current, operation.key, liveRef.current)) {
              showToast(`Couldn't advance playbook: ${String(error)}`, 'error');
            }
          });
        } else if (result.kind === 'clipboard_error') {
          showToast(`Couldn't copy playbook step: ${result.message}`, 'error');
        }
      });
    },
    [activeCopy, isSessionActive, showToast],
  );

  const handleSkip = useCallback(async () => {
    if (!isSessionActive) return;
    try {
      await api.session.advance();
    } catch (err) {
      showToast(`Couldn't skip playbook step: ${String(err)}`, 'error');
    }
  }, [isSessionActive, showToast]);

  function getStepStatus(index: number): StepStatus {
    if (!isSessionActive) return 'pending';
    if (index < currentStep) return 'completed';
    if (index === currentStep) return 'active';
    return 'pending';
  }

  if (loading) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: 13 }}
      >
        Loading playbook...
      </div>
    );
  }

  if (fetchError || sessionError) {
    return (
      <div
        role="alert"
        className="flex-1 flex flex-col items-center justify-center gap-2"
        style={{ color: 'var(--text-secondary)', fontSize: 13 }}
      >
        <span>Couldn't load playbook</span>
        <button type="button" onClick={triggerRefresh}>
          Retry
        </button>
      </div>
    );
  }

  if (!playbook) {
    return (
      <div
        className="flex-1 flex items-center justify-center"
        style={{ color: 'var(--text-secondary)', fontSize: 13 }}
      >
        Playbook not found
      </div>
    );
  }

  const totalSteps = steps.length;
  const completedSteps = isSessionActive ? Math.min(currentStep, totalSteps) : 0;
  const progress = getPlaybookProgress(completedSteps, totalSteps);
  const allComplete = isSessionActive && completedSteps >= totalSteps;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div
        className="flex-shrink-0"
        style={{
          padding: '16px 20px 14px',
          borderBottom: '1px solid var(--border)',
          background: 'var(--bg-secondary)',
        }}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="flex-1 min-w-0">
            <h2
              className="truncate"
              style={{
                fontSize: 18,
                fontWeight: 700,
                color: 'var(--text-primary)',
                margin: 0,
              }}
            >
              {playbook.title}
            </h2>
            {playbook.description && (
              <p
                style={{
                  fontSize: 12,
                  color: 'var(--text-secondary)',
                  margin: '4px 0 0',
                  lineHeight: 1.4,
                }}
              >
                {playbook.description}
              </p>
            )}
          </div>

          {/* Right side: status badge + actions */}
          <div className="flex items-center gap-3 flex-shrink-0">
            <button
              type="button"
              onClick={() => setPlaybookBuilderMode('edit')}
              style={{
                padding: '5px 10px',
                fontSize: 11,
                borderRadius: 6,
                border: '1px solid var(--border)',
                background: 'transparent',
                color: 'var(--text-secondary)',
              }}
            >
              Edit
            </button>
            {isSessionActive ? (
              <>
                <span
                  className="inline-flex items-center gap-1.5 rounded-full"
                  style={{
                    padding: '4px 12px',
                    fontSize: 11,
                    fontWeight: 600,
                    background: allComplete
                      ? 'color-mix(in srgb, #34c759 15%, transparent)'
                      : 'color-mix(in srgb, var(--accent) 12%, transparent)',
                    color: allComplete ? '#34c759' : 'var(--accent)',
                  }}
                >
                  <span
                    style={{
                      width: 6,
                      height: 6,
                      borderRadius: '50%',
                      background: allComplete ? '#34c759' : 'var(--accent)',
                      display: 'inline-block',
                    }}
                  />
                  {allComplete ? 'Complete' : 'In Progress'}
                </span>
                <button
                  className="cursor-default outline-none"
                  style={{
                    fontSize: 12,
                    fontWeight: 500,
                    border: 'none',
                    background: 'transparent',
                    color: '#ff3b30',
                    padding: '4px 8px',
                    borderRadius: 6,
                    transition: 'background 0.15s ease',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background =
                      'color-mix(in srgb, #ff3b30 10%, transparent)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                  onClick={handleEndSession}
                >
                  End Session
                </button>
              </>
            ) : (
              <>
                <span
                  className="inline-flex items-center rounded-full"
                  style={{
                    padding: '4px 12px',
                    fontSize: 11,
                    fontWeight: 600,
                    background: 'color-mix(in srgb, var(--text-secondary) 10%, transparent)',
                    color: 'var(--text-secondary)',
                  }}
                >
                  Not Started
                </span>
                {totalSteps > 0 && (
                  <button
                    className="cursor-default outline-none"
                    style={{
                      padding: '6px 16px',
                      fontSize: 12,
                      fontWeight: 600,
                      border: 'none',
                      borderRadius: 8,
                      background: 'var(--accent)',
                      color: '#ffffff',
                      transition: 'opacity 0.15s ease',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.opacity = '0.85';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.opacity = '1';
                    }}
                    onClick={handleStartSession}
                  >
                    Start Session
                  </button>
                )}
              </>
            )}
          </div>
        </div>

        {/* Progress bar */}
        {isSessionActive && totalSteps > 0 && (
          <div
            className="overflow-hidden rounded-full"
            style={{
              height: 4,
              marginTop: 12,
              background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
            }}
          >
            <div
              className="h-full rounded-full"
              style={{
                width: `${progress * 100}%`,
                background: allComplete ? '#34c759' : 'var(--accent)',
                transition: 'width 0.3s ease, background 0.3s ease',
              }}
            />
          </div>
        )}
      </div>

      {/* Step list */}
      <div className="flex-1 overflow-y-auto" style={{ padding: '20px 24px' }}>
        {steps.length === 0 ? (
          <div
            className="flex items-center justify-center"
            style={{
              padding: 40,
              color: 'var(--text-secondary)',
              fontSize: 13,
            }}
          >
            No steps in this playbook yet. Add prompts to build your workflow.
          </div>
        ) : (
          <div>
            {steps.map((step, idx) => (
              <PlaybookStep
                key={step.id}
                step={step}
                status={getStepStatus(idx)}
                stepNumber={idx + 1}
                isLast={idx === steps.length - 1}
                onCopy={handleCopyAndAdvance}
                onSkip={handleSkip}
                busy={activeCopy !== null && getStepStatus(idx) === 'active'}
              />
            ))}
          </div>
        )}

        {/* Completion message */}
        {allComplete && (
          <div
            className="text-center rounded-lg"
            style={{
              padding: '20px 16px',
              marginTop: 16,
              background: 'color-mix(in srgb, #34c759 8%, transparent)',
              border: '1px solid color-mix(in srgb, #34c759 20%, transparent)',
            }}
          >
            <div style={{ fontSize: 20, marginBottom: 6 }}>
              <svg
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                stroke="#34c759"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                style={{ display: 'inline-block' }}
              >
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <polyline points="22 4 12 14.01 9 11.01" />
              </svg>
            </div>
            <div style={{ fontSize: 14, fontWeight: 600, color: '#34c759' }}>
              Playbook Complete
            </div>
            <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginTop: 4 }}>
              All {totalSteps} steps have been copied. You can end the session or review steps above.
            </div>
          </div>
        )}
      </div>

      {/* Footer */}
      <div
        className="flex items-center justify-between flex-shrink-0"
        style={{
          padding: '10px 20px',
          borderTop: '1px solid var(--border)',
          fontSize: 11,
          color: 'var(--text-secondary)',
        }}
      >
        <span>{totalSteps} step{totalSteps !== 1 ? 's' : ''}</span>
        {isSessionActive && (
          <span>
            {completedSteps} of {totalSteps} completed
          </span>
        )}
      </div>
      {fillCopy?.operation.fill && (
        <FillVariablesModal
          id="fill-playbook-variables"
          content={fillCopy.content}
          names={fillCopy.operation.fill.names}
          onConfirm={(values) => {
            fillCopy.operation.fill?.resume(values);
            setFillCopy(null);
          }}
          onCancel={() => {
            fillCopy.operation.fill?.cancel();
            setFillCopy(null);
          }}
        />
      )}
    </div>
  );
}
