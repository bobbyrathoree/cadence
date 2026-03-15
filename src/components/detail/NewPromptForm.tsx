import { useState, useRef, useEffect, useCallback } from 'react';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';

export function NewPromptForm() {
  const { setIsCreating, setSelectedPromptId, triggerRefresh } = useAppContext();
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [tagsInput, setTagsInput] = useState('');
  const [saving, setSaving] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    titleRef.current?.focus();
  }, []);

  const handleSave = useCallback(async () => {
    const trimmedTitle = title.trim();
    const trimmedContent = content.trim();
    if (!trimmedTitle || !trimmedContent) return;

    setSaving(true);
    try {
      const tags = tagsInput
        .split(',')
        .map((t) => t.trim())
        .filter((t) => t.length > 0);

      const created = await api.prompts.create({
        title: trimmedTitle,
        content: trimmedContent,
        tags: tags.length > 0 ? tags : undefined,
      });

      triggerRefresh();
      setSelectedPromptId(created.id);
      setIsCreating(false);
    } catch (err) {
      console.error('Failed to create prompt:', err);
    } finally {
      setSaving(false);
    }
  }, [title, content, tagsInput, triggerRefresh, setSelectedPromptId, setIsCreating]);

  const handleCancel = useCallback(() => {
    setIsCreating(false);
  }, [setIsCreating]);

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      const meta = e.metaKey || e.ctrlKey;

      if (meta && e.key.toLowerCase() === 's') {
        e.preventDefault();
        handleSave();
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        handleCancel();
        return;
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handleSave, handleCancel]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div
        className="flex-shrink-0"
        style={{
          padding: '16px 20px 12px',
          borderBottom: '1px solid var(--border)',
          background: 'var(--bg-secondary)',
        }}
      >
        <div className="flex items-center gap-3">
          <h2
            className="flex-1"
            style={{
              fontSize: '16px',
              fontWeight: 600,
              color: 'var(--text-primary)',
              margin: 0,
            }}
          >
            New Prompt
          </h2>
          <button
            onClick={handleCancel}
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
            disabled={saving || !title.trim() || !content.trim()}
            className="flex items-center gap-1.5 rounded cursor-default outline-none"
            style={{
              padding: '6px 16px',
              fontSize: '12px',
              fontWeight: 500,
              border: 'none',
              background:
                saving || !title.trim() || !content.trim()
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
        </div>
      </div>

      {/* Form body */}
      <div className="flex-1 overflow-y-auto" style={{ padding: '20px' }}>
        {/* Title */}
        <div style={{ marginBottom: 16 }}>
          <input
            ref={titleRef}
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Prompt title"
            style={{
              width: '100%',
              fontSize: '15px',
              fontWeight: 600,
              color: 'var(--text-primary)',
              background: 'transparent',
              border: 'none',
              borderBottom: '1px solid var(--border)',
              outline: 'none',
              padding: '8px 0',
              transition: 'border-color 0.15s ease',
            }}
            onFocus={(e) => {
              e.currentTarget.style.borderBottomColor = 'var(--accent)';
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderBottomColor = 'var(--border)';
            }}
          />
        </div>

        {/* Tags */}
        <div style={{ marginBottom: 16 }}>
          <input
            type="text"
            value={tagsInput}
            onChange={(e) => setTagsInput(e.target.value)}
            placeholder="Tags (comma-separated)"
            style={{
              width: '100%',
              fontSize: '12px',
              color: 'var(--text-secondary)',
              background: 'transparent',
              border: 'none',
              borderBottom: '1px solid var(--border)',
              outline: 'none',
              padding: '6px 0',
              transition: 'border-color 0.15s ease',
            }}
            onFocus={(e) => {
              e.currentTarget.style.borderBottomColor = 'var(--accent)';
            }}
            onBlur={(e) => {
              e.currentTarget.style.borderBottomColor = 'var(--border)';
            }}
          />
        </div>

        {/* Content */}
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Write your prompt content here..."
          style={{
            width: '100%',
            minHeight: 'calc(100% - 100px)',
            fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
            fontSize: '12px',
            lineHeight: 1.7,
            color: 'var(--text-primary)',
            background: 'transparent',
            border: 'none',
            outline: 'none',
            resize: 'none',
            padding: 0,
          }}
        />
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
        <span>{content.length.toLocaleString()} characters</span>
        {tagsInput.trim() && (
          <span>
            {tagsInput
              .split(',')
              .filter((t) => t.trim()).length}{' '}
            tag
            {tagsInput.split(',').filter((t) => t.trim()).length !== 1 ? 's' : ''}
          </span>
        )}
      </div>
    </div>
  );
}
