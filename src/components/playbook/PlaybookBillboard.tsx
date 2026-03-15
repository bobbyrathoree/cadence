/**
 * PlaybookBillboard -- Feature discovery empty state for Playbooks.
 * Shown when the user clicks into the Playbooks section but has none created yet.
 */
export function PlaybookBillboard() {
  return (
    <div
      className="flex-1 flex items-center justify-center"
      style={{ padding: 32 }}
    >
      <div style={{ maxWidth: 400, textAlign: 'center' }}>
        {/* Icon */}
        <div
          className="mx-auto flex items-center justify-center"
          style={{
            width: 56,
            height: 56,
            borderRadius: 16,
            background: 'color-mix(in srgb, var(--accent) 12%, transparent)',
            marginBottom: 20,
          }}
        >
          <svg
            width="28"
            height="28"
            viewBox="0 0 24 24"
            fill="none"
            stroke="var(--accent)"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M9 5H7a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2h-2" />
            <rect x="9" y="3" width="6" height="4" rx="1" />
            <path d="M9 12h6" />
            <path d="M9 16h6" />
          </svg>
        </div>

        {/* Title */}
        <h2
          style={{
            fontSize: 20,
            fontWeight: 700,
            color: 'var(--text-primary)',
            margin: '0 0 8px',
          }}
        >
          Playbooks
        </h2>

        {/* Subtitle */}
        <p
          style={{
            fontSize: 14,
            lineHeight: 1.5,
            color: 'var(--text-primary)',
            margin: '0 0 8px',
            fontWeight: 500,
          }}
        >
          Turn your prompts into step-by-step workflows. Playbooks guide you
          through your most complex AI sessions.
        </p>

        {/* Description */}
        <p
          style={{
            fontSize: 13,
            lineHeight: 1.5,
            color: 'var(--text-secondary)',
            margin: '0 0 24px',
          }}
        >
          Chain prompts in the right order. Add operator notes. Branch with
          choice steps. Never forget a step again.
        </p>

        {/* CTA Button */}
        <button
          className="cursor-default outline-none"
          style={{
            padding: '10px 24px',
            fontSize: 14,
            fontWeight: 600,
            border: 'none',
            borderRadius: 10,
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
            console.log('[Cadence] Build your first Playbook -- create flow not yet implemented');
          }}
        >
          Build your first Playbook
        </button>
      </div>
    </div>
  );
}
