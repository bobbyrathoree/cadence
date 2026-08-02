import { useEffect, useState, useCallback, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { AppProvider, useAppContext } from './lib/context';
import { api } from './lib/api';
import { usePrompts, useKeyboardShortcuts } from './lib/hooks';
import { eventToBinding } from './lib/keys';
import { getPrimaryVariant } from './lib/prompt';
import { Sidebar } from './components/sidebar/Sidebar';
import { PromptList } from './components/prompt-list/PromptList';
import { DetailPanel } from './components/detail/DetailPanel';
import { Toast } from './components/shared/Toast';
import { ImportModal } from './components/import/ImportModal';
import { SettingsModal } from './components/settings/SettingsModal';

function AppContent() {
  const {
    activeView,
    activeCollectionId,
    selectedPromptId,
    setSelectedPromptId,
    refreshCounter,
    triggerRefresh,
    isCreating,
    setIsCreating,
    isEditing,
    setIsEditing,
    requestEditExit,
    isImportOpen,
    setIsImportOpen,
    isSettingsOpen,
    setIsSettingsOpen,
  } = useAppContext();

  const { prompts } = usePrompts(activeView, activeCollectionId, refreshCounter);
  const { shortcuts } = useKeyboardShortcuts(refreshCounter);

  // Toast state
  const [toast, setToast] = useState({ message: '', visible: false });
  const showToast = useCallback((message: string) => {
    setToast({ message, visible: true });
  }, []);
  const hideToast = useCallback(() => {
    setToast((prev) => ({ ...prev, visible: false }));
  }, []);

  // Build reverse lookup map: binding -> action (skip global shortcuts handled by Rust)
  const shortcutMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of shortcuts) {
      if (s.binding && !s.is_global) {
        map.set(s.binding, s.action);
      }
    }
    return map;
  }, [shortcuts]);

  // Keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      // Ignore keyboard shortcuts when typing in inputs/textareas
      const target = e.target as HTMLElement;
      const tag = target.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable) return;

      const binding = eventToBinding(e);
      const action = shortcutMap.get(binding);
      if (!action) return;

      e.preventDefault();
      switch (action) {
        case 'focus_search': {
          const searchInput = document.querySelector<HTMLInputElement>(
            'input[placeholder="Search prompts..."]',
          );
          if (searchInput) {
            searchInput.focus();
            searchInput.select();
          }
          break;
        }
        case 'new_prompt': {
          requestEditExit(() => setIsCreating(true));
          break;
        }
        case 'toggle_favorite': {
          if (selectedPromptId) {
            api.prompts
              .toggleFavorite(selectedPromptId)
              .then((isFav) => {
                showToast(isFav ? 'Added to Favorites' : 'Removed from Favorites');
                triggerRefresh();
              })
              .catch((err) => console.error('Toggle favorite failed:', err));
          }
          break;
        }
        case 'toggle_edit': {
          if (selectedPromptId && !isCreating) {
            if (isEditing) {
              requestEditExit();
            } else {
              setIsEditing(true);
            }
          }
          break;
        }
        case 'open_import': {
          setIsImportOpen(true);
          break;
        }
        case 'open_settings': {
          setIsSettingsOpen(true);
          break;
        }
        case 'copy_selected': {
          if (!selectedPromptId) return;
          api.prompts
            .get(selectedPromptId)
            .then(async (prompt) => {
              const primaryVariant = getPrimaryVariant(prompt);

              if (primaryVariant) {
                await writeText(primaryVariant.content);
                await api.prompts.recordCopy(prompt.id, primaryVariant.id);
                showToast('Copied to clipboard');
                triggerRefresh();
              }
            })
            .catch((err) => console.error('Copy shortcut failed:', err));
          break;
        }
        case 'deselect': {
          requestEditExit(() => setSelectedPromptId(null));
          break;
        }
        case 'navigate_up': {
          if (prompts.length === 0) return;
          const currentIndex = prompts.findIndex(
            (p) => p.id === selectedPromptId,
          );
          const nextIndex = currentIndex < 0 ? 0 : Math.max(currentIndex - 1, 0);
          const nextId = prompts[nextIndex].id;
          if (nextId !== selectedPromptId) {
            requestEditExit(() => setSelectedPromptId(nextId));
          }
          break;
        }
        case 'navigate_down': {
          if (prompts.length === 0) return;
          const currentIdx = prompts.findIndex(
            (p) => p.id === selectedPromptId,
          );
          const nextIdx = currentIdx < 0 ? 0 : Math.min(currentIdx + 1, prompts.length - 1);
          const nextId = prompts[nextIdx].id;
          if (nextId !== selectedPromptId) {
            requestEditExit(() => setSelectedPromptId(nextId));
          }
          break;
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedPromptId, prompts, shortcutMap, setSelectedPromptId, showToast, triggerRefresh, isCreating, setIsCreating, isEditing, setIsEditing, requestEditExit, setIsImportOpen, setIsSettingsOpen]);

  // Listen for cross-window "db-changed" events from Tauri
  useEffect(() => {
    const unlisten = listen('db-changed', () => {
      triggerRefresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [triggerRefresh]);

  // Listen for "shortcuts-changed" events from Tauri
  useEffect(() => {
    const unlisten = listen('shortcuts-changed', () => {
      triggerRefresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [triggerRefresh]);

  return (
    <div
      className="flex h-screen overflow-hidden"
      style={{ background: 'var(--bg-primary)', color: 'var(--text-primary)' }}
    >
      <Sidebar />
      <PromptList />
      <DetailPanel />
      <Toast message={toast.message} visible={toast.visible} onHide={hideToast} />
      <ImportModal isOpen={isImportOpen} onClose={() => setIsImportOpen(false)} />
      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        shortcuts={shortcuts}
        onUpdateShortcut={async (action, binding) => {
          try {
            await api.settings.updateShortcut(action, binding);
            triggerRefresh();
          } catch (err) {
            showToast(String(err));
          }
        }}
        onResetAll={async () => {
          await api.settings.resetShortcuts();
          triggerRefresh();
        }}
      />
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <AppContent />
    </AppProvider>
  );
}
