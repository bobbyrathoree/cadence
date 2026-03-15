import { useState, useRef } from 'react';
import { api } from '../../lib/api';

interface Props {
  content: string;
  promptId: string;
  variantId?: string;
  onCopy?: () => void;
}

export function CopyButton({ content, promptId, variantId, onCopy }: Props) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  async function handleCopy() {
    try {
      // Record the copy event (also returns content but we already have it)
      await api.prompts.recordCopy(promptId, variantId);
      // Write to clipboard
      await navigator.clipboard.writeText(content);

      setCopied(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopied(false), 1500);

      onCopy?.();
    } catch (err) {
      console.error('Copy failed:', err);
    }
  }

  return (
    <button
      onClick={handleCopy}
      className="flex items-center gap-1.5 rounded cursor-default outline-none"
      style={{
        padding: '6px 16px',
        fontSize: '12px',
        fontWeight: 500,
        border: 'none',
        background: copied ? '#34c759' : 'var(--accent)',
        color: '#ffffff',
        borderRadius: 6,
        transition: 'background 0.15s ease',
      }}
    >
      {copied ? (
        <>Copied &#10003;</>
      ) : (
        <>
          Copy
          <kbd
            style={{
              fontSize: '10px',
              opacity: 0.7,
              fontFamily: 'inherit',
            }}
          >
            {'\u2318'}C
          </kbd>
        </>
      )}
    </button>
  );
}
