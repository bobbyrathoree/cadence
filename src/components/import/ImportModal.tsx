import { useState, useRef, useCallback, useEffect } from 'react';
import { api } from '../../lib/api';
import { useAppContext } from '../../lib/context';
import { PromptSlicer } from './PromptSlicer';
import type { ImportResult } from '../../lib/types';

interface ImportModalProps {
  isOpen: boolean;
  onClose: () => void;
}

type Tab = 'json' | 'markdown' | 'slicer';

export function ImportModal({ isOpen, onClose }: ImportModalProps) {
  const { triggerRefresh } = useAppContext();
  const [activeTab, setActiveTab] = useState<Tab>('json');

  // JSON tab state
  const [jsonText, setJsonText] = useState('');
  const [jsonPreviewCount, setJsonPreviewCount] = useState<number | null>(null);
  const [jsonError, setJsonError] = useState('');
  const jsonFileRef = useRef<HTMLInputElement>(null);

  // Markdown tab state
  const [mdFiles, setMdFiles] = useState<Array<{ name: string; content: string }>>([]);
  const mdFileRef = useRef<HTMLInputElement>(null);

  // Shared
  const [importing, setImporting] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);

  // Parse JSON to preview count
  const handleJsonChange = useCallback((value: string) => {
    setJsonText(value);
    setResult(null);
    setJsonError('');
    if (!value.trim()) {
      setJsonPreviewCount(null);
      return;
    }
    try {
      const parsed = JSON.parse(value);
      if (Array.isArray(parsed)) {
        setJsonPreviewCount(parsed.length);
      } else if (parsed && typeof parsed === 'object' && Array.isArray(parsed.prompts)) {
        setJsonPreviewCount(parsed.prompts.length);
      } else {
        setJsonPreviewCount(1);
      }
    } catch {
      setJsonPreviewCount(null);
      setJsonError('Invalid JSON');
    }
  }, []);

  // Read JSON file
  function handleJsonFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const text = ev.target?.result as string;
      handleJsonChange(text);
    };
    reader.readAsText(file);
    // Reset file input so same file can be re-selected
    e.target.value = '';
  }

  // Import JSON
  async function handleJsonImport() {
    if (!jsonText.trim()) return;
    setImporting(true);
    setResult(null);
    try {
      const res = await api.importExport.importJson(jsonText);
      setResult(res);
      triggerRefresh();
    } catch (err) {
      setResult({ imported: 0, skipped: 0, errors: [String(err)] });
    } finally {
      setImporting(false);
    }
  }

  // Read markdown files
  function handleMdFilesChange(e: React.ChangeEvent<HTMLInputElement>) {
    const files = e.target.files;
    if (!files || files.length === 0) return;

    const filePromises: Promise<{ name: string; content: string }>[] = [];

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      filePromises.push(
        new Promise((resolve) => {
          const reader = new FileReader();
          reader.onload = (ev) => {
            resolve({
              name: file.name,
              content: ev.target?.result as string,
            });
          };
          reader.readAsText(file);
        })
      );
    }

    Promise.all(filePromises).then((results) => {
      setMdFiles((prev) => [...prev, ...results]);
      setResult(null);
    });
    e.target.value = '';
  }

  // Import markdown files
  async function handleMdImport() {
    if (mdFiles.length === 0) return;
    setImporting(true);
    setResult(null);
    try {
      const filePairs: Array<[string, string]> = mdFiles.map((f) => [f.name, f.content]);
      const res = await api.importExport.importMarkdownFiles(filePairs);
      setResult(res);
      triggerRefresh();
    } catch (err) {
      setResult({ imported: 0, skipped: 0, errors: [String(err)] });
    } finally {
      setImporting(false);
    }
  }

  function handleRemoveMdFile(index: number) {
    setMdFiles((prev) => prev.filter((_, i) => i !== index));
    setResult(null);
  }

  // Escape key closes modal
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const tabs: { key: Tab; label: string }[] = [
    { key: 'json', label: 'JSON' },
    { key: 'markdown', label: 'Markdown' },
    { key: 'slicer', label: 'Prompt Slicer' },
  ];

  return (
    // Backdrop
    <div
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 9000,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'rgba(0, 0, 0, 0.4)',
        backdropFilter: 'blur(4px)',
        WebkitBackdropFilter: 'blur(4px)',
        animation: 'modalFadeIn 0.15s ease-out',
      }}
    >
      {/* Modal card */}
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 600,
          maxHeight: '80vh',
          display: 'flex',
          flexDirection: 'column',
          background: 'var(--bg-secondary)',
          borderRadius: 12,
          boxShadow: '0 24px 80px rgba(0, 0, 0, 0.35), 0 0 0 1px rgba(255,255,255,0.05)',
          overflow: 'hidden',
          animation: 'modalSlideIn 0.2s ease-out',
        }}
      >
        {/* Header */}
        <div
          className="flex items-center justify-between flex-shrink-0"
          style={{
            padding: '16px 20px 12px',
            borderBottom: '1px solid var(--border)',
          }}
        >
          <h2
            style={{
              fontSize: '16px',
              fontWeight: 600,
              color: 'var(--text-primary)',
              margin: 0,
            }}
          >
            Import Prompts
          </h2>
          <button
            onClick={onClose}
            style={{
              width: 28,
              height: 28,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 6,
              border: 'none',
              background: 'transparent',
              color: 'var(--text-secondary)',
              cursor: 'default',
              fontSize: '16px',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 15%, transparent)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
            }}
          >
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
              <line x1="4" y1="4" x2="12" y2="12" />
              <line x1="12" y1="4" x2="4" y2="12" />
            </svg>
          </button>
        </div>

        {/* Tab bar — segmented control style */}
        <div
          className="flex-shrink-0"
          style={{ padding: '12px 20px 0' }}
        >
          <div
            className="flex"
            style={{
              background: 'color-mix(in srgb, var(--text-secondary) 10%, transparent)',
              borderRadius: 8,
              padding: 2,
              gap: 2,
            }}
          >
            {tabs.map((tab) => (
              <button
                key={tab.key}
                onClick={() => {
                  setActiveTab(tab.key);
                  setResult(null);
                }}
                style={{
                  flex: 1,
                  padding: '6px 12px',
                  fontSize: '12px',
                  fontWeight: activeTab === tab.key ? 600 : 500,
                  color: activeTab === tab.key ? 'var(--text-primary)' : 'var(--text-secondary)',
                  background: activeTab === tab.key ? 'var(--bg-secondary)' : 'transparent',
                  border: 'none',
                  borderRadius: 6,
                  cursor: 'default',
                  boxShadow: activeTab === tab.key ? '0 1px 3px rgba(0,0,0,0.12)' : 'none',
                  transition: 'all 0.15s ease',
                }}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        {/* Content area */}
        <div
          className="flex-1 overflow-y-auto"
          style={{
            padding: '16px 20px',
            minHeight: 300,
          }}
        >
          {/* JSON Tab */}
          {activeTab === 'json' && (
            <div className="flex flex-col gap-3">
              <textarea
                value={jsonText}
                onChange={(e) => handleJsonChange(e.target.value)}
                placeholder="Paste JSON here..."
                style={{
                  width: '100%',
                  height: 200,
                  fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
                  fontSize: '12px',
                  lineHeight: 1.6,
                  color: 'var(--text-primary)',
                  background: 'color-mix(in srgb, var(--text-secondary) 5%, transparent)',
                  border: '1px solid var(--border)',
                  borderRadius: 8,
                  outline: 'none',
                  resize: 'vertical',
                  padding: '12px',
                }}
                onFocus={(e) => { e.currentTarget.style.borderColor = 'var(--accent)'; }}
                onBlur={(e) => { e.currentTarget.style.borderColor = 'var(--border)'; }}
              />

              <div className="flex items-center gap-3">
                <input
                  ref={jsonFileRef}
                  type="file"
                  accept=".json"
                  onChange={handleJsonFileChange}
                  style={{ display: 'none' }}
                />
                <button
                  onClick={() => jsonFileRef.current?.click()}
                  style={{
                    padding: '6px 14px',
                    fontSize: '12px',
                    fontWeight: 500,
                    color: 'var(--text-secondary)',
                    background: 'transparent',
                    border: '1px solid var(--border)',
                    borderRadius: 6,
                    cursor: 'default',
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 8%, transparent)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  Or select a JSON file
                </button>

                {jsonPreviewCount !== null && !jsonError && (
                  <span style={{ fontSize: '12px', color: 'var(--accent)', fontWeight: 500 }}>
                    Found {jsonPreviewCount} prompt{jsonPreviewCount !== 1 ? 's' : ''}
                  </span>
                )}
                {jsonError && (
                  <span style={{ fontSize: '12px', color: '#ff453a', fontWeight: 500 }}>
                    {jsonError}
                  </span>
                )}
              </div>

              <div className="flex justify-end">
                <button
                  onClick={handleJsonImport}
                  disabled={importing || !jsonText.trim() || !!jsonError}
                  style={{
                    padding: '8px 20px',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: '#ffffff',
                    background:
                      importing || !jsonText.trim() || !!jsonError
                        ? 'color-mix(in srgb, var(--accent) 40%, transparent)'
                        : 'var(--accent)',
                    border: 'none',
                    borderRadius: 8,
                    cursor: 'default',
                    transition: 'background 0.15s ease',
                  }}
                >
                  {importing ? 'Importing...' : 'Import'}
                </button>
              </div>
            </div>
          )}

          {/* Markdown Tab */}
          {activeTab === 'markdown' && (
            <div className="flex flex-col gap-3">
              <div
                style={{
                  fontSize: '12px',
                  color: 'var(--text-secondary)',
                  lineHeight: 1.5,
                }}
              >
                Select one or more markdown (.md) files. Each file will become a prompt,
                with the filename as the title.
              </div>

              <div className="flex items-center gap-3">
                <input
                  ref={mdFileRef}
                  type="file"
                  accept=".md,.markdown"
                  multiple
                  onChange={handleMdFilesChange}
                  style={{ display: 'none' }}
                />
                <button
                  onClick={() => mdFileRef.current?.click()}
                  style={{
                    padding: '8px 16px',
                    fontSize: '12px',
                    fontWeight: 500,
                    color: 'var(--text-primary)',
                    background: 'transparent',
                    border: '1px solid var(--border)',
                    borderRadius: 6,
                    cursor: 'default',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 6,
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 8%, transparent)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M2 4.5V12a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H8.5L7 3H3a1 1 0 0 0-1 1.5z" />
                  </svg>
                  Select markdown files
                </button>
              </div>

              {/* File list */}
              {mdFiles.length > 0 && (
                <div
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 4,
                    maxHeight: 200,
                    overflowY: 'auto',
                    padding: '8px',
                    background: 'color-mix(in srgb, var(--text-secondary) 5%, transparent)',
                    borderRadius: 8,
                    border: '1px solid var(--border)',
                  }}
                >
                  {mdFiles.map((file, idx) => (
                    <div
                      key={`${file.name}-${idx}`}
                      className="flex items-center justify-between"
                      style={{
                        padding: '6px 8px',
                        fontSize: '12px',
                        color: 'var(--text-primary)',
                        background: 'var(--bg-secondary)',
                        borderRadius: 4,
                      }}
                    >
                      <div className="flex items-center gap-2 truncate flex-1">
                        <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                          <path d="M9 1H4a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V5L9 1z" />
                          <polyline points="9 1 9 5 13 5" />
                        </svg>
                        <span className="truncate">{file.name}</span>
                        <span style={{ fontSize: '10px', color: 'var(--text-secondary)', flexShrink: 0 }}>
                          {(file.content.length / 1024).toFixed(1)} KB
                        </span>
                      </div>
                      <button
                        onClick={() => handleRemoveMdFile(idx)}
                        style={{
                          width: 20,
                          height: 20,
                          display: 'flex',
                          alignItems: 'center',
                          justifyContent: 'center',
                          borderRadius: 4,
                          border: 'none',
                          background: 'transparent',
                          color: 'var(--text-secondary)',
                          cursor: 'default',
                          flexShrink: 0,
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.background = 'color-mix(in srgb, #ff453a 15%, transparent)';
                          e.currentTarget.style.color = '#ff453a';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = 'transparent';
                          e.currentTarget.style.color = 'var(--text-secondary)';
                        }}
                      >
                        <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                          <line x1="4" y1="4" x2="12" y2="12" />
                          <line x1="12" y1="4" x2="4" y2="12" />
                        </svg>
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {mdFiles.length > 0 && (
                <div className="flex items-center justify-between">
                  <span style={{ fontSize: '12px', color: 'var(--accent)', fontWeight: 500 }}>
                    {mdFiles.length} file{mdFiles.length !== 1 ? 's' : ''} selected
                  </span>
                  <button
                    onClick={handleMdImport}
                    disabled={importing}
                    style={{
                      padding: '8px 20px',
                      fontSize: '13px',
                      fontWeight: 600,
                      color: '#ffffff',
                      background: importing
                        ? 'color-mix(in srgb, var(--accent) 40%, transparent)'
                        : 'var(--accent)',
                      border: 'none',
                      borderRadius: 8,
                      cursor: 'default',
                      transition: 'background 0.15s ease',
                    }}
                  >
                    {importing ? 'Importing...' : 'Import'}
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Prompt Slicer Tab */}
          {activeTab === 'slicer' && (
            <div style={{ height: 350 }}>
              <PromptSlicer />
            </div>
          )}
        </div>

        {/* Footer — import results */}
        {result && (
          <div
            className="flex-shrink-0"
            style={{
              padding: '12px 20px',
              borderTop: '1px solid var(--border)',
              fontSize: '12px',
              display: 'flex',
              alignItems: 'center',
              gap: 12,
            }}
          >
            {result.imported > 0 && (
              <span style={{ color: '#34c759', fontWeight: 600 }}>
                Imported {result.imported}
              </span>
            )}
            {result.skipped > 0 && (
              <span style={{ color: 'var(--text-secondary)', fontWeight: 500 }}>
                Skipped {result.skipped}
              </span>
            )}
            {result.errors.length > 0 && (
              <span style={{ color: '#ff453a', fontWeight: 500 }}>
                {result.errors.length} error{result.errors.length !== 1 ? 's' : ''}
              </span>
            )}
            {result.imported > 0 && result.errors.length === 0 && result.skipped === 0 && (
              <span style={{ color: 'var(--text-secondary)' }}>
                All prompts imported successfully
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
