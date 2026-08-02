import { useState } from 'react';
import { useAppContext } from '../../lib/context';
import type { ActiveView } from '../../lib/context';
import {
  useCollections,
  usePlaybooks,
  useTags,
  usePlaybookSession,
  usePromptCounts,
} from '../../lib/hooks';
import { api } from '../../lib/api';
import { CollectionItem } from './CollectionItem';

/* ------------------------------------------------------------------ */
/*  Inline SVG icons — lightweight, no external dependency            */
/* ------------------------------------------------------------------ */

function ClipboardIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="4" y="2" width="8" height="12" rx="1.5" />
      <path d="M6 2V1.5A.5.5 0 0 1 6.5 1h3a.5.5 0 0 1 .5.5V2" />
      <line x1="6.5" y1="6" x2="9.5" y2="6" />
      <line x1="6.5" y1="8.5" x2="9.5" y2="8.5" />
    </svg>
  );
}

function StarIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round">
      <path d="M8 1.5l1.85 3.75 4.15.6-3 2.93.71 4.12L8 10.88 4.29 12.9 5 8.78 2 5.85l4.15-.6z" />
    </svg>
  );
}

function ClockIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="8" cy="8" r="6" />
      <path d="M8 4.5V8l2.5 1.5" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2 4.5V12a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1H8.5L7 3H3a1 1 0 0 0-1 1.5z" />
    </svg>
  );
}

function ListIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <line x1="5" y1="4" x2="13" y2="4" />
      <line x1="5" y1="8" x2="13" y2="8" />
      <line x1="5" y1="12" x2="13" y2="12" />
      <circle cx="2.5" cy="4" r="0.8" fill="currentColor" stroke="none" />
      <circle cx="2.5" cy="8" r="0.8" fill="currentColor" stroke="none" />
      <circle cx="2.5" cy="12" r="0.8" fill="currentColor" stroke="none" />
    </svg>
  );
}

/* ------------------------------------------------------------------ */
/*  Section header                                                     */
/* ------------------------------------------------------------------ */

function SectionHeader({ children, trailing }: { children: React.ReactNode; trailing?: React.ReactNode }) {
  return (
    <div
      className="flex items-center justify-between px-2.5 pt-4 pb-1 select-none"
      style={{
        fontSize: '10px',
        fontWeight: 600,
        letterSpacing: '0.06em',
        textTransform: 'uppercase' as const,
        color: 'var(--text-secondary)',
      }}
    >
      <span>{children}</span>
      {trailing}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Colored dot for smart collections                                  */
/* ------------------------------------------------------------------ */

function ColorDot({ color }: { color?: string | null }) {
  return (
    <span
      className="inline-block flex-shrink-0 rounded-full"
      style={{
        width: 8,
        height: 8,
        backgroundColor: color ?? 'var(--accent)',
      }}
    />
  );
}

/* ------------------------------------------------------------------ */
/*  Main Sidebar                                                       */
/* ------------------------------------------------------------------ */

function ImportIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M8 2v8" />
      <path d="M5 7l3 3 3-3" />
      <path d="M2 12v1a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1v-1" />
    </svg>
  );
}

function ExportIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M8 10V2" />
      <path d="M5 5l3-3 3 3" />
      <path d="M2 12v1a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1v-1" />
    </svg>
  );
}

function GearIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="8" r="2.5" />
      <path d="M13.3 9.7a1.2 1.2 0 0 0 .24 1.32l.04.04a1.44 1.44 0 1 1-2.04 2.04l-.04-.04a1.2 1.2 0 0 0-1.32-.24 1.2 1.2 0 0 0-.72 1.1v.12a1.44 1.44 0 1 1-2.88 0v-.06a1.2 1.2 0 0 0-.78-1.1 1.2 1.2 0 0 0-1.32.24l-.04.04a1.44 1.44 0 1 1-2.04-2.04l.04-.04a1.2 1.2 0 0 0 .24-1.32 1.2 1.2 0 0 0-1.1-.72H1.44a1.44 1.44 0 1 1 0-2.88h.06a1.2 1.2 0 0 0 1.1-.78 1.2 1.2 0 0 0-.24-1.32l-.04-.04a1.44 1.44 0 1 1 2.04-2.04l.04.04a1.2 1.2 0 0 0 1.32.24h.06a1.2 1.2 0 0 0 .72-1.1V1.44a1.44 1.44 0 1 1 2.88 0v.06a1.2 1.2 0 0 0 .72 1.1 1.2 1.2 0 0 0 1.32-.24l.04-.04a1.44 1.44 0 1 1 2.04 2.04l-.04.04a1.2 1.2 0 0 0-.24 1.32v.06a1.2 1.2 0 0 0 1.1.72h.12a1.44 1.44 0 1 1 0 2.88h-.06a1.2 1.2 0 0 0-1.1.72z" />
    </svg>
  );
}

export function Sidebar() {
  const {
    activeView,
    setActiveView,
    activeCollectionId,
    setActiveCollectionId,
    activePlaybookId,
    setActivePlaybookId,
    playbookBuilderMode,
    setPlaybookBuilderMode,
    requestEditExit,
    refreshCounter,
    setIsImportOpen,
    setIsSettingsOpen,
    showToast,
  } = useAppContext();

  const [exporting, setExporting] = useState(false);
  const [creatingCollection, setCreatingCollection] = useState(false);
  const [collectionName, setCollectionName] = useState('');
  const [collectionError, setCollectionError] = useState<string | null>(null);
  const [savingCollection, setSavingCollection] = useState(false);

  const { data: collections, error: collectionsError } =
    useCollections(refreshCounter);
  const { data: playbooks, error: playbooksError } =
    usePlaybooks(refreshCounter);
  const { data: tags, error: tagsError } = useTags(refreshCounter);
  const { data: session, error: sessionError } =
    usePlaybookSession(refreshCounter);
  const { data: counts, error: countsError } =
    usePromptCounts(refreshCounter);

  const allCount = counts?.all;
  const favCount = counts?.favorites;
  const recentCount = counts?.recents;

  const smartCollections = collections.filter((c) => c.is_smart);
  const regularCollections = collections.filter((c) => !c.is_smart);

  function handleViewClick(view: ActiveView) {
    requestEditExit(() => {
      setActiveView(view);
      setActiveCollectionId(null);
      setActivePlaybookId(null);
      setPlaybookBuilderMode(null);
    });
  }

  function handleCollectionClick(id: string) {
    requestEditExit(() => {
      setActiveView('collection');
      setActiveCollectionId(id);
      setActivePlaybookId(null);
      setPlaybookBuilderMode(null);
    });
  }

  function handlePlaybookClick(id: string) {
    requestEditExit(() => {
      setActiveView('playbook');
      setActivePlaybookId(id);
      setActiveCollectionId(null);
      setPlaybookBuilderMode(null);
    });
  }

  async function createManualCollection() {
    const name = collectionName.trim();
    if (!name || savingCollection) return;
    setSavingCollection(true);
    setCollectionError(null);
    try {
      const collection = await api.collections.create({
        name,
        is_smart: false,
      });
      setCollectionName('');
      setCreatingCollection(false);
      setActiveView('collection');
      setActiveCollectionId(collection.id);
      setActivePlaybookId(null);
      setPlaybookBuilderMode(null);
    } catch (error) {
      setCollectionError(String(error));
    } finally {
      setSavingCollection(false);
    }
  }

  return (
    <aside
      className="flex flex-col flex-shrink-0 overflow-y-auto select-none"
      style={{
        width: 220,
        background: 'var(--bg-sidebar)',
        borderRight: '1px solid var(--border)',
      }}
    >
      {/* macOS drag region */}
      <div className="h-9 flex-shrink-0" data-tauri-drag-region="" />

      {/* Built-in views */}
      <div className="flex flex-col gap-0.5 px-2">
        <CollectionItem
          icon={<ClipboardIcon />}
          name="All Prompts"
          count={allCount}
          isActive={activeView === 'all'}
          onClick={() => handleViewClick('all')}
        />
        <CollectionItem
          icon={<StarIcon />}
          name="Favorites"
          count={favCount}
          isActive={activeView === 'favorites'}
          onClick={() => handleViewClick('favorites')}
        />
        <CollectionItem
          icon={<ClockIcon />}
          name="Recently Copied"
          count={recentCount}
          isActive={activeView === 'recents'}
          onClick={() => handleViewClick('recents')}
        />
        {countsError && (
          <div role="alert" className="px-2.5 py-1" style={{ fontSize: 10, color: '#ff453a' }}>
            Couldn't load counts
          </div>
        )}
      </div>

      {/* Smart Collections */}
      {smartCollections.length > 0 && (
        <div className="px-2">
          <SectionHeader>Smart Collections</SectionHeader>
          <div className="flex flex-col gap-0.5">
            {smartCollections.map((c) => (
              <CollectionItem
                key={c.id}
                icon={<ColorDot color={c.color} />}
                name={c.name}
                isActive={activeView === 'collection' && activeCollectionId === c.id}
                onClick={() => handleCollectionClick(c.id)}
              />
            ))}
          </div>
        </div>
      )}

      {/* Regular Collections */}
      <div className="px-2">
        <SectionHeader
          trailing={
            <button
              type="button"
              aria-label="New Collection"
              title="New Collection"
              onClick={() =>
                requestEditExit(() => {
                  setCollectionError(null);
                  setCreatingCollection((visible) => !visible);
                })
              }
              style={sectionAddButtonStyle}
            >
              +
            </button>
          }
        >
          Collections
        </SectionHeader>
        {creatingCollection && (
          <form
            className="flex gap-1 px-1 pb-1"
            onSubmit={(event) => {
              event.preventDefault();
              void createManualCollection();
            }}
          >
            <input
              autoFocus
              aria-label="Collection name"
              value={collectionName}
              onChange={(event) => setCollectionName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Escape') {
                  event.preventDefault();
                  setCreatingCollection(false);
                  setCollectionError(null);
                }
              }}
              placeholder="Collection name"
              style={{
                minWidth: 0,
                flex: 1,
                height: 28,
                border: '1px solid var(--border)',
                borderRadius: 5,
                padding: '4px 7px',
                background: 'var(--bg-primary)',
                color: 'var(--text-primary)',
                fontSize: 11,
              }}
            />
            <button
              type="submit"
              disabled={!collectionName.trim() || savingCollection}
              style={{
                height: 28,
                padding: '0 8px',
                border: 0,
                borderRadius: 5,
                background: 'var(--accent)',
                color: '#ffffff',
                fontSize: 11,
              }}
            >
              Add
            </button>
          </form>
        )}
        {collectionError && (
          <div role="alert" className="px-2.5 pb-1" style={{ fontSize: 10, color: '#ff453a' }}>
            {collectionError}
          </div>
        )}
        {collectionsError ? (
          <div role="alert" className="px-2.5 py-1" style={{ fontSize: 11, color: '#ff453a' }}>
            Couldn't load collections
          </div>
        ) : regularCollections.length === 0 ? (
          <div
            className="px-2.5 py-1"
            style={{ fontSize: '11px', color: 'var(--text-secondary)' }}
          >
            No collections yet
          </div>
        ) : (
          <div className="flex flex-col gap-0.5">
            {regularCollections.map((c) => (
              <CollectionItem
                key={c.id}
                icon={<FolderIcon />}
                name={c.name}
                isActive={activeView === 'collection' && activeCollectionId === c.id}
                onClick={() => handleCollectionClick(c.id)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Playbooks */}
      <div className="px-2">
        <SectionHeader
          trailing={
            <button
              type="button"
              aria-label="New Playbook"
              title="New Playbook"
              onClick={() => {
                requestEditExit(() => {
                  setActiveView('playbook');
                  setActiveCollectionId(null);
                  setActivePlaybookId(null);
                  setPlaybookBuilderMode('create');
                });
              }}
              style={{
                width: 22,
                height: 22,
                border: 'none',
                borderRadius: 5,
                background:
                  playbookBuilderMode === 'create'
                    ? 'color-mix(in srgb, var(--accent) 18%, transparent)'
                    : 'transparent',
                color: 'var(--accent)',
                fontSize: 16,
                lineHeight: 1,
              }}
            >
              +
            </button>
          }
        >
          Playbooks
        </SectionHeader>
        {playbooksError || sessionError ? (
          <div role="alert" className="px-2.5 py-1" style={{ fontSize: 11, color: '#ff453a' }}>
            Couldn't load playbooks
          </div>
        ) : playbooks.length === 0 ? (
          <div
            className="px-2.5 py-1"
            style={{ fontSize: '11px', color: 'var(--text-secondary)' }}
          >
            No playbooks yet
          </div>
        ) : (
          <div className="flex flex-col gap-0.5">
            {playbooks.map((pb) => {
              const isSessionActive =
                session?.active_playbook_id === pb.id;
              const isSelected =
                activeView === 'playbook' && activePlaybookId === pb.id;

              return (
                <div key={pb.id}>
                  <CollectionItem
                    icon={<ListIcon />}
                    name={pb.title}
                    isActive={isSelected}
                    onClick={() => handlePlaybookClick(pb.id)}
                  />
                  {isSessionActive && session && (
                    <div className="px-2.5 pb-1">
                      <div
                        className="h-1 rounded-full overflow-hidden"
                        style={{ background: 'color-mix(in srgb, var(--accent) 20%, transparent)' }}
                      >
                        <div
                          className="h-full rounded-full transition-all duration-300"
                          style={{
                            width: `${Math.max(5, (session.current_step + 1) * 20)}%`,
                            background: 'var(--accent)',
                          }}
                        />
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Tags */}
      <div className="px-2 mt-auto">
        <SectionHeader>Tags</SectionHeader>
        {tagsError ? (
          <div role="alert" className="px-2.5 py-1" style={{ fontSize: 11, color: '#ff453a' }}>
            Couldn't load tags
          </div>
        ) : tags.length === 0 ? (
          <div
            className="px-2.5 py-1"
            style={{ fontSize: '11px', color: 'var(--text-secondary)' }}
          >
            No tags yet
          </div>
        ) : (
          <div className="flex flex-wrap gap-1 px-1 pt-1">
            {tags.slice(0, 8).map((tag) => (
              <span
                key={tag.id}
                className="inline-block rounded-full px-2 py-0.5 truncate max-w-[90px]"
                style={{
                  fontSize: '10px',
                  background: tag.color
                    ? `color-mix(in srgb, ${tag.color} 15%, transparent)`
                    : 'color-mix(in srgb, var(--text-secondary) 12%, transparent)',
                  color: tag.color ?? 'var(--text-secondary)',
                }}
              >
                {tag.name}
              </span>
            ))}
          </div>
        )}
      </div>

      {/* Settings */}
      <div
        className="flex-shrink-0 px-2 pt-2"
        style={{ borderTop: '1px solid var(--border)', marginTop: 8 }}
      >
        <button
          onClick={() => setIsSettingsOpen(true)}
          className="flex items-center gap-2 w-full rounded-md cursor-default"
          style={{
            padding: '6px 10px',
            fontSize: '11px',
            fontWeight: 500,
            color: 'var(--text-secondary)',
            background: 'transparent',
            border: '1px solid var(--border)',
            borderRadius: 6,
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 10%, transparent)';
            e.currentTarget.style.color = 'var(--text-primary)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent';
            e.currentTarget.style.color = 'var(--text-secondary)';
          }}
        >
          <GearIcon />
          Settings
        </button>
      </div>

      {/* Import / Export */}
      <div
        className="flex-shrink-0 px-2 pb-3 pt-1.5"
      >
        <div className="flex gap-1.5">
          <button
            onClick={() => setIsImportOpen(true)}
            className="flex items-center justify-center gap-1.5 flex-1 rounded-md cursor-default"
            style={{
              padding: '6px 0',
              fontSize: '11px',
              fontWeight: 500,
              color: 'var(--text-secondary)',
              background: 'transparent',
              border: '1px solid var(--border)',
              borderRadius: 6,
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 10%, transparent)';
              e.currentTarget.style.color = 'var(--text-primary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.color = 'var(--text-secondary)';
            }}
          >
            <ImportIcon />
            Import
          </button>
          <button
            onClick={async () => {
              if (exporting) return;
              setExporting(true);
              try {
                const jsonStr = await api.importExport.exportJson();
                const blob = new Blob([jsonStr], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const date = new Date().toISOString().split('T')[0];
                const a = document.createElement('a');
                a.href = url;
                a.download = `cadence-export-${date}.json`;
                document.body.appendChild(a);
                a.click();
                document.body.removeChild(a);
                URL.revokeObjectURL(url);
              } catch (err) {
                showToast(`Couldn't export prompts: ${String(err)}`, 'error');
              } finally {
                setExporting(false);
              }
            }}
            disabled={exporting}
            className="flex items-center justify-center gap-1.5 flex-1 rounded-md cursor-default"
            style={{
              padding: '6px 0',
              fontSize: '11px',
              fontWeight: 500,
              color: 'var(--text-secondary)',
              background: 'transparent',
              border: '1px solid var(--border)',
              borderRadius: 6,
              opacity: exporting ? 0.5 : 1,
            }}
            onMouseEnter={(e) => {
              if (!exporting) {
                e.currentTarget.style.background = 'color-mix(in srgb, var(--text-secondary) 10%, transparent)';
                e.currentTarget.style.color = 'var(--text-primary)';
              }
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.color = 'var(--text-secondary)';
            }}
          >
            <ExportIcon />
            {exporting ? 'Exporting...' : 'Export'}
          </button>
        </div>
      </div>
    </aside>
  );
}

const sectionAddButtonStyle: React.CSSProperties = {
  width: 22,
  height: 22,
  border: 'none',
  borderRadius: 5,
  background: 'transparent',
  color: 'var(--accent)',
  fontSize: 16,
  lineHeight: 1,
};
