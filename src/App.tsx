import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { AppProvider, useAppContext } from './lib/context';
import { api } from './lib/api';
import {
  canApplyCopyEffect,
  startCopy,
  type CopyOperation,
} from './lib/copy';
import {
  useCollectionPrompts,
  useKeyboardShortcuts,
  usePrompts,
} from './lib/hooks';
import { eventToBinding, shouldIgnoreShortcutFromTarget } from './lib/keys';
import { getPrimaryVariant } from './lib/prompt';
import { Sidebar } from './components/sidebar/Sidebar';
import { PromptList } from './components/prompt-list/PromptList';
import { DetailPanel } from './components/detail/DetailPanel';
import { Toast } from './components/shared/Toast';
import { ImportModal } from './components/import/ImportModal';
import { SettingsModal } from './components/settings/SettingsModal';
import { FillVariablesModal } from './components/shared/FillVariablesModal';

function AppContent() {
  const [lastApiError, setLastApiError] = useState<string | null>(null);
  const {
    activeView,
    activeCollectionId,
    selectedPromptId,
    setSelectedPromptId,
    selectedVariantByPrompt,
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
    displayedPromptIds,
    hasOpenModal,
    toast,
    showToast,
    hideToast,
  } = useAppContext();
  const [activeCopy, setActiveCopy] = useState<CopyOperation | null>(null);
  const [fillCopy, setFillCopy] = useState<{
    operation: CopyOperation;
    content: string;
  } | null>(null);
  const copyRef = useRef<CopyOperation | null>(null);
  const liveRef = useRef(true);
  const selectedPromptIdRef = useRef(selectedPromptId);
  selectedPromptIdRef.current = selectedPromptId;

  useEffect(() => {
    liveRef.current = true;
    return () => {
      liveRef.current = false;
      copyRef.current?.fill?.cancel();
      copyRef.current = null;
    };
  }, []);

  useEffect(() => {
    copyRef.current?.fill?.cancel();
    copyRef.current = null;
    setActiveCopy(null);
    setFillCopy(null);
  }, [selectedPromptId]);

  const beginAppCopy = useCallback(
    (content: string, promptId: string, variantId: string) => {
      if (activeCopy) return;
      const operation = startCopy({
        content,
        promptId,
        variantId,
        onAccounting: (ok, key) => {
          if (!canApplyCopyEffect(copyRef.current, key, liveRef.current)) return;
          if (!ok) {
            showToast("Copied, but couldn't record usage", 'error');
          }
        },
      });
      copyRef.current = operation;
      setActiveCopy(operation);
      if (operation.fill) setFillCopy({ operation, content });

      void operation.settled.then((result) => {
        if (!canApplyCopyEffect(copyRef.current, operation.key, liveRef.current)) {
          return;
        }
        setActiveCopy(null);
        setFillCopy(null);
        if (result.kind === 'copied') {
          showToast('Copied to clipboard');
        } else if (result.kind === 'clipboard_error') {
          showToast(`Couldn't copy prompt: ${result.message}`, 'error');
        }
      });
    },
    [activeCopy, showToast],
  );

  const promptFilter =
    activeView === 'favorites'
      ? 'favorites'
      : activeView === 'recents'
        ? 'recent'
        : 'all';
  const {
    data: allPrompts,
    error: allPromptsError,
    loading: allPromptsLoading,
    hasMore: allPromptsHasMore,
    loadingMore: allPromptsLoadingMore,
    loadMore: loadMorePrompts,
  } = usePrompts(refreshCounter, promptFilter);
  const {
    data: collectionPrompts,
    error: collectionPromptsError,
    loading: collectionPromptsLoading,
    hasMore: collectionPromptsHasMore,
    loadingMore: collectionPromptsLoadingMore,
    loadMore: loadMoreCollectionPrompts,
  } = useCollectionPrompts(
    activeView === 'collection' ? activeCollectionId : null,
    refreshCounter,
  );
  const prompts =
    activeView === 'collection' ? collectionPrompts : allPrompts;
  const promptsLoading =
    activeView === 'collection' ? collectionPromptsLoading : allPromptsLoading;
  const promptsError =
    activeView === 'collection' ? collectionPromptsError : allPromptsError;
  const promptsHasMore =
    activeView === 'collection' ? collectionPromptsHasMore : allPromptsHasMore;
  const promptsLoadingMore =
    activeView === 'collection'
      ? collectionPromptsLoadingMore
      : allPromptsLoadingMore;
  const loadMore =
    activeView === 'collection'
      ? loadMoreCollectionPrompts
      : loadMorePrompts;
  const { data: shortcuts } = useKeyboardShortcuts(refreshCounter);

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
      if (hasOpenModal) return;
      if (shouldIgnoreShortcutFromTarget(e.target, e)) return;

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
              })
              .catch((err) =>
                showToast(`Couldn't update favorite: ${String(err)}`, 'error'),
              );
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
          if (!selectedPromptId || activeCopy) return;
          const requestedPromptId = selectedPromptId;
          api.prompts
            .get(requestedPromptId)
            .then((prompt) => {
              if (
                !liveRef.current ||
                selectedPromptIdRef.current !== requestedPromptId
              ) {
                return;
              }
              const registeredVariant =
                selectedVariantByPrompt?.promptId === prompt.id
                  ? prompt.variants.find(
                      (variant) =>
                        variant.id === selectedVariantByPrompt.variantId,
                    )
                  : null;
              const variant = registeredVariant ?? getPrimaryVariant(prompt);
              if (variant) {
                beginAppCopy(variant.content, prompt.id, variant.id);
              }
            })
            .catch((err) => {
              if (
                liveRef.current &&
                selectedPromptIdRef.current === requestedPromptId
              ) {
                showToast(`Couldn't copy prompt: ${String(err)}`, 'error');
              }
            });
          break;
        }
        case 'deselect': {
          requestEditExit(() => setSelectedPromptId(null));
          break;
        }
        case 'navigate_up': {
          if (displayedPromptIds.length === 0) return;
          const currentIndex = displayedPromptIds.indexOf(selectedPromptId ?? '');
          const nextIndex = currentIndex < 0 ? 0 : Math.max(currentIndex - 1, 0);
          const nextId = displayedPromptIds[nextIndex];
          if (nextId !== selectedPromptId) {
            requestEditExit(() => setSelectedPromptId(nextId));
          }
          break;
        }
        case 'navigate_down': {
          if (displayedPromptIds.length === 0) return;
          const currentIdx = displayedPromptIds.indexOf(selectedPromptId ?? '');
          const nextIdx =
            currentIdx < 0
              ? 0
              : Math.min(currentIdx + 1, displayedPromptIds.length - 1);
          const nextId = displayedPromptIds[nextIdx];
          if (nextId !== selectedPromptId) {
            requestEditExit(() => setSelectedPromptId(nextId));
          }
          break;
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedPromptId, selectedVariantByPrompt, displayedPromptIds, shortcutMap, setSelectedPromptId, showToast, isCreating, setIsCreating, isEditing, setIsEditing, requestEditExit, setIsImportOpen, setIsSettingsOpen, hasOpenModal, activeCopy, beginAppCopy]);

  // Listen for cross-window "db-changed" events from Tauri
  useEffect(() => {
    const unlisten = listen('db-changed', () => {
      triggerRefresh();
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [triggerRefresh]);

  useEffect(() => {
    const unlisten = listen<string>('api-error', (event) => {
      const message = `Local API failed to start: ${event.payload}`;
      setLastApiError(message);
      showToast(message, 'error');
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [showToast]);

  return (
    <div
      className="flex h-screen overflow-hidden"
      aria-busy={activeCopy !== null}
      style={{ background: 'var(--bg-primary)', color: 'var(--text-primary)' }}
    >
      <Sidebar />
      <PromptList
        prompts={prompts}
        promptsLoading={promptsLoading}
        promptsError={promptsError}
        onRetry={triggerRefresh}
        hasMore={promptsHasMore}
        loadingMore={promptsLoadingMore}
        onLoadMore={loadMore}
      />
      <DetailPanel prompts={allPrompts} />
      <Toast
        message={toast.message}
        visible={toast.visible}
        tone={toast.tone}
        onHide={hideToast}
      />
      <ImportModal isOpen={isImportOpen} onClose={() => setIsImportOpen(false)} />
      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        apiError={lastApiError}
        shortcuts={shortcuts}
        onUpdateShortcut={async (action, binding) => {
          try {
            await api.settings.updateShortcut(action, binding);
          } catch (err) {
            showToast(String(err));
          }
        }}
        onResetAll={async () => {
          await api.settings.resetShortcuts();
        }}
      />
      {fillCopy?.operation.fill && (
        <FillVariablesModal
          id="fill-shortcut-variables"
          content={fillCopy.content}
          names={fillCopy.operation.fill.names}
          onConfirm={(values) => {
            fillCopy.operation.fill?.resume(values);
            setFillCopy(null);
          }}
          onCancel={() => {
            fillCopy.operation.fill?.cancel();
            setFillCopy(null);
          }}
        />
      )}
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
