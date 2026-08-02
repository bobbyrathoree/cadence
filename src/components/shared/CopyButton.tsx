import { useState, useRef } from 'react';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';

interface Props {
  content: string;
  promptId: string;
  variantId?: string;
  onCopy?: () => void;
}

export function CopyButton({ content, promptId, variantId, onCopy }: Props) {
  const { showToast } = useAppContext();
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  async function handleCopy() {
    try {
      // Record the copy event (also returns content but we already have it)
      await api.prompts.recordCopy(promptId, variantId);
      // Write to clipboard via Tauri plugin
      await writeText(content);

      setCopied(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => setCopied(false), 1500);

      onCopy?.();
    } catch (err) {
      showToast(`Couldn't copy prompt: ${String(err)}`, 'error');
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
