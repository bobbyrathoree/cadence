import React, {
  createContext,
  useContext,
  useState,
  useCallback,
  useMemo,
} from 'react';
import {
  PROMPT_EDIT_EXIT_EVENT,
  type PromptEditExitDetail,
} from './promptDrafts';

export type ActiveView = 'all' | 'favorites' | 'recents' | 'collection' | 'playbook';
export type PlaybookBuilderMode = 'create' | 'edit';
export type ToastTone = 'default' | 'error';

export interface ToastState {
  message: string;
  visible: boolean;
  tone: ToastTone;
}

export interface AppContextType {
  // Navigation
  activeView: ActiveView;
  setActiveView: (view: ActiveView) => void;
  activeCollectionId: string | null;
  setActiveCollectionId: (id: string | null) => void;
  activePlaybookId: string | null;
  setActivePlaybookId: (id: string | null) => void;
  playbookBuilderMode: PlaybookBuilderMode | null;
  setPlaybookBuilderMode: (mode: PlaybookBuilderMode | null) => void;

  // Selection
  selectedPromptId: string | null;
  setSelectedPromptId: (id: string | null) => void;

  // Search
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  displayedPromptIds: string[];
  setDisplayedPromptIds: (ids: string[]) => void;

  // Refresh trigger
  refreshCounter: number;
  triggerRefresh: () => void;

  // Create / Edit modes
  isCreating: boolean;
  setIsCreating: (v: boolean) => void;
  isEditing: boolean;
  setIsEditing: (v: boolean) => void;
  requestEditExit: (afterExit?: () => void) => void;

  // Import modal
  isImportOpen: boolean;
  setIsImportOpen: (v: boolean) => void;

  // Settings modal
  isSettingsOpen: boolean;
  setIsSettingsOpen: (v: boolean) => void;

  // Modal stack
  hasOpenModal: boolean;
  registerModal: (id: string) => void;
  unregisterModal: (id: string) => void;
  isTopModal: (id: string) => boolean;

  // Notifications
  toast: ToastState;
  showToast: (message: string, tone?: ToastTone) => void;
  hideToast: () => void;
}

const AppContext = createContext<AppContextType | null>(null);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [activeView, setActiveView] = useState<ActiveView>('all');
  const [activeCollectionId, setActiveCollectionId] = useState<string | null>(null);
  const [activePlaybookId, setActivePlaybookId] = useState<string | null>(null);
  const [playbookBuilderMode, setPlaybookBuilderMode] =
    useState<PlaybookBuilderMode | null>(null);
  const [selectedPromptId, setSelectedPromptId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [displayedPromptIds, setDisplayedPromptIds] = useState<string[]>([]);
  const [refreshCounter, setRefreshCounter] = useState(0);
  const [isCreating, setIsCreating] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isImportOpen, setIsImportOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [modalStack, setModalStack] = useState<string[]>([]);
  const [toast, setToast] = useState<ToastState>({
    message: '',
    visible: false,
    tone: 'default',
  });

  const triggerRefresh = useCallback(() => {
    setRefreshCounter((c) => c + 1);
  }, []);

  const requestEditExit = useCallback(
    (afterExit: () => void = () => undefined) => {
      if (!isEditing) {
        afterExit();
        return;
      }
      window.dispatchEvent(
        new CustomEvent<PromptEditExitDetail>(PROMPT_EDIT_EXIT_EVENT, {
          detail: { afterExit },
        }),
      );
    },
    [isEditing],
  );

  const registerModal = useCallback((id: string) => {
    setModalStack((current) => [...current.filter((entry) => entry !== id), id]);
  }, []);

  const unregisterModal = useCallback((id: string) => {
    setModalStack((current) => current.filter((entry) => entry !== id));
  }, []);

  const isTopModal = useCallback(
    (id: string) => modalStack[modalStack.length - 1] === id,
    [modalStack],
  );

  const showToast = useCallback(
    (message: string, tone: ToastTone = 'default') => {
      setToast({ message, visible: true, tone });
    },
    [],
  );

  const hideToast = useCallback(() => {
    setToast((current) => ({ ...current, visible: false }));
  }, []);

  const value = useMemo<AppContextType>(
    () => ({
      activeView,
      setActiveView,
      activeCollectionId,
      setActiveCollectionId,
      activePlaybookId,
      setActivePlaybookId,
      playbookBuilderMode,
      setPlaybookBuilderMode,
      selectedPromptId,
      setSelectedPromptId,
      searchQuery,
      setSearchQuery,
      displayedPromptIds,
      setDisplayedPromptIds,
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
      hasOpenModal: modalStack.length > 0,
      registerModal,
      unregisterModal,
      isTopModal,
      toast,
      showToast,
      hideToast,
    }),
    [
      activeView,
      activeCollectionId,
      activePlaybookId,
      playbookBuilderMode,
      selectedPromptId,
      searchQuery,
      displayedPromptIds,
      refreshCounter,
      triggerRefresh,
      isCreating,
      isEditing,
      requestEditExit,
      isImportOpen,
      isSettingsOpen,
      modalStack,
      registerModal,
      unregisterModal,
      isTopModal,
      toast,
      showToast,
      hideToast,
    ],
  );

  return (
    <AppContext.Provider value={value}>{children}</AppContext.Provider>
  );
}

export function useAppContext(): AppContextType {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error('useAppContext must be used within <AppProvider>');
  }
  return ctx;
}
