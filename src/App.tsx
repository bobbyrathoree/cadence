import { useEffect, useState, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { AppProvider, useAppContext } from './lib/context';
import { api } from './lib/api';
import { usePrompts } from './lib/hooks';
import { Sidebar } from './components/sidebar/Sidebar';
import { PromptList } from './components/prompt-list/PromptList';
import { DetailPanel } from './components/detail/DetailPanel';
import { Toast } from './components/shared/Toast';

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
  } = useAppContext();

  const { prompts } = usePrompts(activeView, activeCollectionId, refreshCounter);

  // Toast state
  const [toast, setToast] = useState({ message: '', visible: false });
  const showToast = useCallback((message: string) => {
    setToast({ message, visible: true });
  }, []);
  const hideToast = useCallback(() => {
    setToast((prev) => ({ ...prev, visible: false }));
  }, []);

  // Keyboard shortcuts
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const meta = e.metaKey || e.ctrlKey;
      const key = e.key.toLowerCase();

      // Ignore keyboard shortcuts when typing in inputs/textareas
      const target = e.target as HTMLElement;
      const isInput =
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable;

      // Cmd+F: focus search input
      if (meta && key === 'f') {
        e.preventDefault();
        const searchInput = document.querySelector<HTMLInputElement>(
          'input[placeholder="Search prompts..."]',
        );
        if (searchInput) {
          searchInput.focus();
          searchInput.select();
        }
        return;
      }

      // Cmd+N: new prompt
      if (meta && key === 'n') {
        e.preventDefault();
        setIsEditing(false);
        setIsCreating(true);
        return;
      }

      // Cmd+D: toggle favorite on selected prompt
      if (meta && key === 'd') {
        e.preventDefault();
        if (selectedPromptId) {
          api.prompts
            .toggleFavorite(selectedPromptId)
            .then((isFav) => {
              showToast(isFav ? 'Added to Favorites' : 'Removed from Favorites');
              triggerRefresh();
            })
            .catch((err) => console.error('Toggle favorite failed:', err));
        }
        return;
      }

      // Cmd+E: toggle edit mode on selected prompt
      if (meta && key === 'e') {
        e.preventDefault();
        if (selectedPromptId && !isCreating) {
          setIsEditing(!isEditing);
        }
        return;
      }

      // Skip arrow / enter / escape when typing in inputs
      if (isInput) return;

      // ArrowUp / ArrowDown: navigate prompt list
      if (key === 'arrowdown' || key === 'arrowup') {
        e.preventDefault();
        if (prompts.length === 0) return;

        const currentIndex = prompts.findIndex(
          (p) => p.id === selectedPromptId,
        );

        let nextIndex: number;
        if (key === 'arrowdown') {
          nextIndex = currentIndex < 0 ? 0 : Math.min(currentIndex + 1, prompts.length - 1);
        } else {
          nextIndex = currentIndex < 0 ? 0 : Math.max(currentIndex - 1, 0);
        }

        setSelectedPromptId(prompts[nextIndex].id);
        return;
      }

      // Enter: copy selected prompt's primary variant
      if (key === 'enter') {
        e.preventDefault();
        if (!selectedPromptId) return;

        api.prompts
          .get(selectedPromptId)
          .then(async (prompt) => {
            const primaryVariant = prompt.variants.find(
              (v) => v.id === prompt.primary_variant_id,
            ) ?? prompt.variants[0];

            if (primaryVariant) {
              await writeText(primaryVariant.content);
              await api.prompts.recordCopy(prompt.id, primaryVariant.id);
              showToast('Copied to clipboard');
              triggerRefresh();
            }
          })
          .catch((err) => console.error('Copy shortcut failed:', err));
        return;
      }

      // Escape: deselect
      if (key === 'escape') {
        setSelectedPromptId(null);
        return;
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedPromptId, prompts, setSelectedPromptId, showToast, triggerRefresh, isCreating, setIsCreating, isEditing, setIsEditing]);

  // Listen for cross-window "db-changed" events from Tauri
  useEffect(() => {
    const unlisten = listen('db-changed', () => {
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
