import { useState, useRef, useEffect, useCallback } from 'react';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';

interface SlicedPrompt {
  title: string;
  startOffset: number;
  endOffset: number;
}

export function PromptSlicer() {
  const { triggerRefresh } = useAppContext();
  const [text, setText] = useState('');
  const [selectedText, setSelectedText] = useState('');
  const [selectionRange, setSelectionRange] = useState<{ start: number; end: number } | null>(null);
  const [floatingPos, setFloatingPos] = useState<{ x: number; y: number } | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [formTitle, setFormTitle] = useState('');
  const [formTags, setFormTags] = useState('');
  const [saving, setSaving] = useState(false);
  const [createdCount, setCreatedCount] = useState(0);
  const [slicedRanges, setSlicedRanges] = useState<SlicedPrompt[]>([]);
  const contentRef = useRef<HTMLDivElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const floatingRef = useRef<HTMLDivElement>(null);

  // Detect text selection within the content area
  const handleMouseUp = useCallback(() => {
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || !contentRef.current) {
      // Don't clear if the form is showing (user might be clicking form elements)
      if (!showForm) {
        setFloatingPos(null);
        setSelectedText('');
        setSelectionRange(null);
      }
      return;
    }

    // Check that selection is within our content div
    const range = selection.getRangeAt(0);
    if (!contentRef.current.contains(range.commonAncestorContainer)) {
      return;
    }

    const selText = selection.toString().trim();
    if (selText.length < 5) {
      return;
    }

    // Get position for floating button
    const rect = range.getBoundingClientRect();
    const containerRect = contentRef.current.getBoundingClientRect();

    setSelectedText(selText);
    setFloatingPos({
      x: rect.left - containerRect.left + rect.width / 2,
      y: rect.top - containerRect.top - 8,
    });

    // Calculate offsets within the full text for highlighting
    const fullText = contentRef.current.textContent || '';
    const startIdx = fullText.indexOf(selText);
    if (startIdx >= 0) {
      setSelectionRange({ start: startIdx, end: startIdx + selText.length });
    }
  }, [showForm]);

  // Close floating button when clicking outside
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (
        floatingRef.current &&
        !floatingRef.current.contains(e.target as Node) &&
        !showForm
      ) {
        // Let mouseup handle it
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [showForm]);

  // Focus title input when form opens
  useEffect(() => {
    if (showForm && titleInputRef.current) {
      titleInputRef.current.focus();
    }
  }, [showForm]);

  function handleCreateClick() {
    setShowForm(true);
    setFormTitle('');
    setFormTags('');
  }

  async function handleSave() {
    const trimmedTitle = formTitle.trim();
    if (!trimmedTitle || !selectedText) return;

    setSaving(true);
    try {
      const tags = formTags
        .split(',')
        .map((t) => t.trim())
        .filter((t) => t.length > 0);

      await api.prompts.create({
        title: trimmedTitle,
        content: selectedText,
        tags: tags.length > 0 ? tags : undefined,
      });

      setCreatedCount((c) => c + 1);
      if (selectionRange) {
        setSlicedRanges((prev) => [
          ...prev,
          { title: trimmedTitle, startOffset: selectionRange.start, endOffset: selectionRange.end },
        ]);
      }
      triggerRefresh();

      // Reset form
      setShowForm(false);
      setFloatingPos(null);
      setSelectedText('');
      setSelectionRange(null);
      window.getSelection()?.removeAllRanges();
    } catch (err) {
      console.error('Failed to create prompt from slice:', err);
    } finally {
      setSaving(false);
    }
  }

  function handleFormCancel() {
    setShowForm(false);
  }

  function handleFormKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSave();
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      handleFormCancel();
    }
  }

  // Render text with highlighted sliced ranges
  function renderHighlightedText() {
    if (!text || slicedRanges.length === 0) {
      return text;
    }

    // Sort ranges by start offset
    const sorted = [...slicedRanges].sort((a, b) => a.startOffset - b.startOffset);
    const parts: React.ReactNode[] = [];
    let lastEnd = 0;

    sorted.forEach((range, idx) => {
      // Text before this range
      if (range.startOffset > lastEnd) {
        parts.push(
          <span key={`text-${idx}`}>{text.slice(lastEnd, range.startOffset)}</span>
        );
      }
      // The highlighted range
      parts.push(
        <span
          key={`highlight-${idx}`}
          style={{
            background: 'color-mix(in srgb, #34c759 18%, transparent)',
            borderRadius: 2,
            borderLeft: '2px solid #34c759',
            paddingLeft: 2,
          }}
          title={`Saved as: "${range.title}"`}
        >
          {text.slice(range.startOffset, range.endOffset)}
        </span>
      );
      lastEnd = range.endOffset;
    });

    // Remaining text
    if (lastEnd < text.length) {
      parts.push(<span key="text-end">{text.slice(lastEnd)}</span>);
    }

    return parts;
  }

  return (
    <div className="flex flex-col gap-3 h-full">
      {/* Instructions */}
      <div
        style={{
          fontSize: '12px',
          color: 'var(--text-secondary)',
          lineHeight: 1.5,
        }}
      >
        Paste text from a ChatGPT conversation or any document. Select text with your mouse,
        then click "Create Prompt" to save it as a new prompt.
      </div>

      {/* Counter */}
      {createdCount > 0 && (
        <div
          style={{
            fontSize: '12px',
            fontWeight: 600,
            color: '#34c759',
            padding: '6px 10px',
            background: 'color-mix(in srgb, #34c759 10%, transparent)',
            borderRadius: 6,
          }}
        >
          {createdCount} prompt{createdCount !== 1 ? 's' : ''} created from this text
        </div>
      )}

      {/* Content area */}
      <div style={{ position: 'relative', flex: 1, minHeight: 0 }}>
        {/* Paste area when empty */}
        {!text ? (
          <textarea
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Paste your text content here (e.g., a ChatGPT conversation export)..."
            style={{
              width: '100%',
              height: '100%',
              fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
              fontSize: '12px',
              lineHeight: 1.7,
              color: 'var(--text-primary)',
              background: 'color-mix(in srgb, var(--text-secondary) 5%, transparent)',
              border: '1px dashed var(--border)',
              borderRadius: 8,
              outline: 'none',
              resize: 'none',
              padding: '12px',
            }}
          />
        ) : (
          <>
            {/* Toolbar */}
            <div
              className="flex items-center justify-between"
              style={{
                marginBottom: 8,
                fontSize: '11px',
                color: 'var(--text-secondary)',
              }}
            >
              <span>{text.length.toLocaleString()} characters pasted</span>
              <button
                onClick={() => {
                  setText('');
                  setSlicedRanges([]);
                  setCreatedCount(0);
                  setFloatingPos(null);
                  setShowForm(false);
                }}
                style={{
                  fontSize: '11px',
                  color: 'var(--text-secondary)',
                  background: 'transparent',
                  border: 'none',
                  cursor: 'default',
                  textDecoration: 'underline',
                  textUnderlineOffset: 2,
                }}
              >
                Clear text
              </button>
            </div>

            {/* Selectable text display */}
            <div
              ref={contentRef}
              onMouseUp={handleMouseUp}
              style={{
                width: '100%',
                height: 'calc(100% - 30px)',
                fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
                fontSize: '12px',
                lineHeight: 1.7,
                color: 'var(--text-primary)',
                background: 'color-mix(in srgb, var(--text-secondary) 5%, transparent)',
                border: '1px solid var(--border)',
                borderRadius: 8,
                padding: '12px',
                overflowY: 'auto',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                userSelect: 'text',
                cursor: 'text',
                position: 'relative',
              }}
            >
              {renderHighlightedText()}

              {/* Floating "Create Prompt" button */}
              {floatingPos && !showForm && (
                <div
                  ref={floatingRef}
                  style={{
                    position: 'absolute',
                    left: Math.max(0, Math.min(floatingPos.x - 70, 460)),
                    top: Math.max(0, floatingPos.y - 36),
                    zIndex: 100,
                    animation: 'floatIn 0.15s ease-out',
                  }}
                >
                  <button
                    onClick={handleCreateClick}
                    style={{
                      padding: '6px 14px',
                      fontSize: '12px',
                      fontWeight: 600,
                      color: '#ffffff',
                      background: 'var(--accent)',
                      border: 'none',
                      borderRadius: 8,
                      cursor: 'default',
                      boxShadow: '0 4px 16px rgba(0, 0, 0, 0.25), 0 0 0 1px rgba(255,255,255,0.1)',
                      whiteSpace: 'nowrap',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 6,
                    }}
                  >
                    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                      <line x1="8" y1="3" x2="8" y2="13" />
                      <line x1="3" y1="8" x2="13" y2="8" />
                    </svg>
                    Create Prompt
                  </button>
                </div>
              )}

              {/* Inline form for title + tags */}
              {showForm && floatingPos && (
                <div
                  style={{
                    position: 'absolute',
                    left: Math.max(0, Math.min(floatingPos.x - 160, 260)),
                    top: Math.max(0, floatingPos.y - 8),
                    zIndex: 100,
                    width: 320,
                    padding: '14px',
                    background: 'var(--bg-secondary)',
                    border: '1px solid var(--border)',
                    borderRadius: 10,
                    boxShadow: '0 8px 32px rgba(0, 0, 0, 0.25)',
                    animation: 'floatIn 0.15s ease-out',
                  }}
                  onKeyDown={handleFormKeyDown}
                >
                  {/* Selected text preview */}
                  <div
                    style={{
                      fontSize: '11px',
                      color: 'var(--text-secondary)',
                      marginBottom: 10,
                      padding: '6px 8px',
                      background: 'color-mix(in srgb, var(--text-secondary) 8%, transparent)',
                      borderRadius: 4,
                      maxHeight: 54,
                      overflow: 'hidden',
                      lineHeight: 1.4,
                      fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
                    }}
                  >
                    {selectedText.slice(0, 200)}{selectedText.length > 200 ? '...' : ''}
                  </div>

                  {/* Title input */}
                  <input
                    ref={titleInputRef}
                    type="text"
                    value={formTitle}
                    onChange={(e) => setFormTitle(e.target.value)}
                    placeholder="Prompt title"
                    style={{
                      width: '100%',
                      fontSize: '13px',
                      fontWeight: 600,
                      color: 'var(--text-primary)',
                      background: 'transparent',
                      border: 'none',
                      borderBottom: '1px solid var(--border)',
                      outline: 'none',
                      padding: '6px 0',
                      marginBottom: 8,
                    }}
                    onFocus={(e) => { e.currentTarget.style.borderBottomColor = 'var(--accent)'; }}
                    onBlur={(e) => { e.currentTarget.style.borderBottomColor = 'var(--border)'; }}
                  />

                  {/* Tags input */}
                  <input
                    type="text"
                    value={formTags}
                    onChange={(e) => setFormTags(e.target.value)}
                    placeholder="Tags (comma-separated)"
                    style={{
                      width: '100%',
                      fontSize: '11px',
                      color: 'var(--text-secondary)',
                      background: 'transparent',
                      border: 'none',
                      borderBottom: '1px solid var(--border)',
                      outline: 'none',
                      padding: '4px 0',
                      marginBottom: 12,
                    }}
                    onFocus={(e) => { e.currentTarget.style.borderBottomColor = 'var(--accent)'; }}
                    onBlur={(e) => { e.currentTarget.style.borderBottomColor = 'var(--border)'; }}
                  />

                  {/* Buttons */}
                  <div className="flex items-center gap-2 justify-end">
                    <button
                      onClick={handleFormCancel}
                      style={{
                        padding: '5px 12px',
                        fontSize: '12px',
                        fontWeight: 500,
                        color: 'var(--text-secondary)',
                        background: 'transparent',
                        border: '1px solid var(--border)',
                        borderRadius: 6,
                        cursor: 'default',
                      }}
                    >
                      Cancel
                    </button>
                    <button
                      onClick={handleSave}
                      disabled={saving || !formTitle.trim()}
                      style={{
                        padding: '5px 14px',
                        fontSize: '12px',
                        fontWeight: 600,
                        color: '#ffffff',
                        background:
                          saving || !formTitle.trim()
                            ? 'color-mix(in srgb, var(--accent) 40%, transparent)'
                            : 'var(--accent)',
                        border: 'none',
                        borderRadius: 6,
                        cursor: 'default',
                      }}
                    >
                      {saving ? 'Saving...' : 'Save'}
                    </button>
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
