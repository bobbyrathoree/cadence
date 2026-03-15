import { useAppContext } from '../../lib/context';
import type { ActiveView } from '../../lib/context';
import {
  usePrompts,
  useCollections,
  usePlaybooks,
  useTags,
  usePlaybookSession,
} from '../../lib/hooks';
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

export function Sidebar() {
  const {
    activeView,
    setActiveView,
    activeCollectionId,
    setActiveCollectionId,
    activePlaybookId,
    setActivePlaybookId,
    refreshCounter,
  } = useAppContext();

  const { prompts } = usePrompts('all', null, refreshCounter);
  const { collections } = useCollections(refreshCounter);
  const { playbooks } = usePlaybooks(refreshCounter);
  const { tags } = useTags(refreshCounter);
  const { session } = usePlaybookSession(refreshCounter);

  const allCount = prompts.length;
  const favCount = prompts.filter((p) => p.is_favorite).length;
  const recentCount = prompts.filter((p) => p.last_copied_at !== null).length;

  const smartCollections = collections.filter((c) => c.is_smart);
  const regularCollections = collections.filter((c) => !c.is_smart);

  function handleViewClick(view: ActiveView) {
    setActiveView(view);
    setActiveCollectionId(null);
    setActivePlaybookId(null);
  }

  function handleCollectionClick(id: string) {
    setActiveView('collection');
    setActiveCollectionId(id);
    setActivePlaybookId(null);
  }

  function handlePlaybookClick(id: string) {
    setActiveView('playbook');
    setActivePlaybookId(id);
    setActiveCollectionId(null);
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
        <SectionHeader>Collections</SectionHeader>
        {regularCollections.length === 0 ? (
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
            <span
              className="rounded-full px-1.5 py-0.5"
              style={{
                fontSize: '9px',
                fontWeight: 600,
                letterSpacing: '0.03em',
                background: 'color-mix(in srgb, var(--accent) 15%, transparent)',
                color: 'var(--accent)',
              }}
            >
              New
            </span>
          }
        >
          Playbooks
        </SectionHeader>
        {playbooks.length === 0 ? (
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
      <div className="px-2 mt-auto pb-3">
        <SectionHeader>Tags</SectionHeader>
        {tags.length === 0 ? (
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
    </aside>
  );
}
