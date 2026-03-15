import { useState, useMemo } from 'react';
import type { PlaybookStepWithPrompt } from '../../lib/types';

export type StepStatus = 'completed' | 'active' | 'pending';

interface Props {
  step: PlaybookStepWithPrompt;
  status: StepStatus;
  stepNumber: number;
  isLast: boolean;
  onCopy: (content: string) => void;
}

/* ------------------------------------------------------------------ */
/*  Step circle indicator                                              */
/* ------------------------------------------------------------------ */

function StepCircle({ status, stepNumber }: { status: StepStatus; stepNumber: number }) {
  if (status === 'completed') {
    return (
      <div
        className="flex items-center justify-center flex-shrink-0"
        style={{
          width: 28,
          height: 28,
          borderRadius: '50%',
          background: '#34c759',
          color: '#fff',
          fontSize: 13,
          fontWeight: 600,
        }}
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
          <polyline points="3 8.5 6.5 12 13 4" />
        </svg>
      </div>
    );
  }

  if (status === 'active') {
    return (
      <div
        className="flex items-center justify-center flex-shrink-0"
        style={{
          width: 28,
          height: 28,
          borderRadius: '50%',
          background: 'var(--accent)',
          color: '#fff',
          fontSize: 12,
          fontWeight: 700,
        }}
      >
        {stepNumber}
      </div>
    );
  }

  // pending
  return (
    <div
      className="flex items-center justify-center flex-shrink-0"
      style={{
        width: 28,
        height: 28,
        borderRadius: '50%',
        border: '2px dashed var(--border)',
        color: 'var(--text-secondary)',
        fontSize: 12,
        fontWeight: 600,
        background: 'transparent',
      }}
    >
      {stepNumber}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Connector line between steps                                       */
/* ------------------------------------------------------------------ */

function ConnectorLine({ status }: { status: StepStatus }) {
  const isCompleted = status === 'completed';
  return (
    <div
      style={{
        width: 2,
        flex: 1,
        minHeight: 16,
        marginLeft: 13, // center under 28px circle
        background: isCompleted
          ? '#34c759'
          : 'var(--border)',
        borderLeft: status === 'pending' ? '2px dashed var(--border)' : undefined,
        ...(status === 'pending' ? { width: 0, background: 'none' } : {}),
      }}
    />
  );
}

/* ------------------------------------------------------------------ */
/*  Variant selector for active step                                   */
/* ------------------------------------------------------------------ */

function StepVariantSelector({
  variants,
  selectedId,
  onSelect,
}: {
  variants: { id: string; label: string }[];
  selectedId: string;
  onSelect: (id: string) => void;
}) {
  if (variants.length <= 1) return null;

  return (
    <div
      className="inline-flex rounded-md overflow-hidden"
      style={{
        background: 'color-mix(in srgb, var(--text-secondary) 10%, transparent)',
        marginBottom: 10,
      }}
    >
      {variants.map((v) => {
        const isActive = v.id === selectedId;
        return (
          <button
            key={v.id}
            onClick={() => onSelect(v.id)}
            className="cursor-default outline-none"
            style={{
              padding: '4px 12px',
              fontSize: 11,
              fontWeight: 500,
              border: 'none',
              borderRadius: 5,
              margin: 2,
              background: isActive ? 'var(--accent)' : 'transparent',
              color: isActive ? '#ffffff' : 'var(--text-secondary)',
              transition: 'all 0.15s ease',
            }}
          >
            {v.label}
          </button>
        );
      })}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Main PlaybookStep                                                  */
/* ------------------------------------------------------------------ */

export function PlaybookStep({ step, status, stepNumber, isLast, onCopy }: Props) {
  const [selectedVariantId, setSelectedVariantId] = useState<string | null>(null);

  // Resolve the variant to display for the active step
  const activeVariant = useMemo(() => {
    if (!step.prompt) return null;
    if (selectedVariantId) {
      return step.prompt.variants.find((v) => v.id === selectedVariantId) ?? step.prompt.variants[0] ?? null;
    }
    const primary = step.prompt.variants.find((v) => v.id === step.prompt!.primary_variant_id);
    return primary ?? step.prompt.variants[0] ?? null;
  }, [step.prompt, selectedVariantId]);

  const stepTitle = step.prompt?.title ?? `Step ${stepNumber}`;
  const isChoice = step.step_type === 'choice';

  return (
    <div className="flex gap-0">
      {/* Left rail: circle + connector */}
      <div className="flex flex-col items-center flex-shrink-0" style={{ width: 28 }}>
        <StepCircle status={status} stepNumber={stepNumber} />
        {!isLast && <ConnectorLine status={status} />}
      </div>

      {/* Right content */}
      <div className="flex-1 min-w-0" style={{ paddingLeft: 14, paddingBottom: isLast ? 0 : 20 }}>
        {/* ---- COMPLETED ---- */}
        {status === 'completed' && (
          <div style={{ paddingTop: 4 }}>
            <div className="flex items-center gap-2">
              <span
                style={{
                  fontSize: 13,
                  fontWeight: 500,
                  color: 'var(--text-secondary)',
                  textDecoration: 'line-through',
                }}
              >
                {stepTitle}
              </span>
              <span
                className="inline-block rounded-full"
                style={{
                  fontSize: 10,
                  fontWeight: 600,
                  padding: '2px 8px',
                  background: 'color-mix(in srgb, #34c759 15%, transparent)',
                  color: '#34c759',
                }}
              >
                Copied
              </span>
            </div>
          </div>
        )}

        {/* ---- ACTIVE ---- */}
        {status === 'active' && (
          <div
            className="rounded-lg"
            style={{
              padding: 16,
              marginTop: 2,
              background: 'color-mix(in srgb, var(--accent) 6%, transparent)',
              border: '1px solid color-mix(in srgb, var(--accent) 20%, transparent)',
            }}
          >
            {/* Step title */}
            <div className="flex items-center gap-2 mb-2">
              <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)' }}>
                {stepTitle}
              </span>
              {isChoice && (
                <span
                  className="inline-block rounded-full"
                  style={{
                    fontSize: 10,
                    fontWeight: 600,
                    padding: '2px 8px',
                    background: 'color-mix(in srgb, #af52de 15%, transparent)',
                    color: '#af52de',
                  }}
                >
                  Choice
                </span>
              )}
            </div>

            {/* Operator notes */}
            {step.instructions && (
              <div
                className="rounded-md"
                style={{
                  padding: '10px 14px',
                  marginBottom: 12,
                  background: 'color-mix(in srgb, #ff9500 8%, transparent)',
                  borderLeft: '3px solid #ff9500',
                  fontSize: 12,
                  lineHeight: 1.6,
                  color: 'var(--text-primary)',
                }}
              >
                <span style={{ fontWeight: 600, color: '#ff9500', fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.04em' }}>
                  Operator Notes
                </span>
                <div style={{ marginTop: 4 }}>{step.instructions}</div>
              </div>
            )}

            {/* CHOICE step: grid of prompt pills */}
            {isChoice && step.choice_prompts && step.choice_prompts.length > 0 && (
              <div className="flex flex-wrap gap-2 mb-3">
                {step.choice_prompts.map((cp) => {
                  const content =
                    cp.variants.find((v) => v.id === cp.primary_variant_id)?.content ??
                    cp.variants[0]?.content ??
                    '';
                  return (
                    <button
                      key={cp.id}
                      className="cursor-default outline-none rounded-lg"
                      style={{
                        padding: '8px 16px',
                        fontSize: 12,
                        fontWeight: 500,
                        border: '1px solid color-mix(in srgb, #af52de 30%, transparent)',
                        background: 'color-mix(in srgb, #af52de 8%, transparent)',
                        color: 'var(--text-primary)',
                        transition: 'all 0.15s ease',
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.background =
                          'color-mix(in srgb, #af52de 18%, transparent)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background =
                          'color-mix(in srgb, #af52de 8%, transparent)';
                      }}
                      onClick={() => onCopy(content)}
                    >
                      {cp.title}
                    </button>
                  );
                })}
              </div>
            )}

            {/* SINGLE step: variant selector + prompt preview + copy button */}
            {!isChoice && step.prompt && (
              <>
                <StepVariantSelector
                  variants={step.prompt.variants}
                  selectedId={selectedVariantId ?? activeVariant?.id ?? ''}
                  onSelect={setSelectedVariantId}
                />

                {/* Content preview */}
                {activeVariant && (
                  <pre
                    className="overflow-y-auto"
                    style={{
                      fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
                      fontSize: 11,
                      lineHeight: 1.6,
                      whiteSpace: 'pre-wrap',
                      wordBreak: 'break-word',
                      color: 'var(--text-primary)',
                      margin: '0 0 12px',
                      maxHeight: 150,
                      padding: '10px 12px',
                      borderRadius: 8,
                      background: 'color-mix(in srgb, var(--text-primary) 4%, transparent)',
                    }}
                  >
                    {activeVariant.content}
                  </pre>
                )}

                {/* Copy button */}
                <button
                  className="cursor-default outline-none"
                  style={{
                    padding: '8px 20px',
                    fontSize: 13,
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
                  onClick={() => {
                    if (activeVariant) onCopy(activeVariant.content);
                  }}
                >
                  Copy Step {stepNumber}
                </button>
              </>
            )}
          </div>
        )}

        {/* ---- PENDING ---- */}
        {status === 'pending' && (
          <div style={{ paddingTop: 5 }}>
            <div className="flex items-center gap-2">
              <span
                style={{
                  fontSize: 13,
                  fontWeight: 500,
                  color: 'var(--text-secondary)',
                }}
              >
                {stepTitle}
              </span>
              {isChoice && (
                <span
                  className="inline-block rounded-full"
                  style={{
                    fontSize: 10,
                    fontWeight: 600,
                    padding: '2px 8px',
                    background: 'color-mix(in srgb, #af52de 10%, transparent)',
                    color: 'color-mix(in srgb, #af52de 60%, var(--text-secondary))',
                  }}
                >
                  Choice
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
