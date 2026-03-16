import React, { createContext, useContext, useState, useCallback } from 'react';

export type ActiveView = 'all' | 'favorites' | 'recents' | 'collection' | 'playbook';

export interface AppContextType {
  // Navigation
  activeView: ActiveView;
  setActiveView: (view: ActiveView) => void;
  activeCollectionId: string | null;
  setActiveCollectionId: (id: string | null) => void;
  activePlaybookId: string | null;
  setActivePlaybookId: (id: string | null) => void;

  // Selection
  selectedPromptId: string | null;
  setSelectedPromptId: (id: string | null) => void;

  // Search
  searchQuery: string;
  setSearchQuery: (query: string) => void;

  // Refresh trigger
  refreshCounter: number;
  triggerRefresh: () => void;

  // Create / Edit modes
  isCreating: boolean;
  setIsCreating: (v: boolean) => void;
  isEditing: boolean;
  setIsEditing: (v: boolean) => void;

  // Import modal
  isImportOpen: boolean;
  setIsImportOpen: (v: boolean) => void;

  // Settings modal
  isSettingsOpen: boolean;
  setIsSettingsOpen: (v: boolean) => void;
}

const AppContext = createContext<AppContextType | null>(null);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const [activeView, setActiveView] = useState<ActiveView>('all');
  const [activeCollectionId, setActiveCollectionId] = useState<string | null>(null);
  const [activePlaybookId, setActivePlaybookId] = useState<string | null>(null);
  const [selectedPromptId, setSelectedPromptId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [refreshCounter, setRefreshCounter] = useState(0);
  const [isCreating, setIsCreating] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [isImportOpen, setIsImportOpen] = useState(false);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  const triggerRefresh = useCallback(() => {
    setRefreshCounter((c) => c + 1);
  }, []);

  return (
    <AppContext.Provider
      value={{
        activeView,
        setActiveView,
        activeCollectionId,
        setActiveCollectionId,
        activePlaybookId,
        setActivePlaybookId,
        selectedPromptId,
        setSelectedPromptId,
        searchQuery,
        setSearchQuery,
        refreshCounter,
        triggerRefresh,
        isCreating,
        setIsCreating,
        isEditing,
        setIsEditing,
        isImportOpen,
        setIsImportOpen,
        isSettingsOpen,
        setIsSettingsOpen,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useAppContext(): AppContextType {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error('useAppContext must be used within <AppProvider>');
  }
  return ctx;
}
