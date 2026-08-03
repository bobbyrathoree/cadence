import { useState, useEffect, useRef, useCallback } from 'react';
import { api } from './api';
import type {
  PromptListItem,
  PromptWithVariants,
  Tag,
  Collection,
  Playbook,
  PlaybookWithSteps,
  PlaybookSession,
  KeyboardShortcut,
  PromptCounts,
  PromptListFilter,
} from './types';

export interface FetchState<T> {
  data: T;
  error: Error | null;
  loading: boolean;
}

export interface PaginatedFetchState<T> extends FetchState<T[]> {
  hasMore: boolean;
  loadingMore: boolean;
  loadMore: () => void;
}

export interface PromptDetailFetchState
  extends FetchState<PromptWithVariants | null> {
  refreshing: boolean;
  refreshError: Error | null;
}

const PAGE_SIZE = 100;

export function mergePromptPages(
  current: PromptListItem[],
  page: PromptListItem[],
): PromptListItem[] {
  const merged = [...current];
  const seen = new Set(current.map((item) => item.id));
  for (const item of page) {
    if (!seen.has(item.id)) {
      seen.add(item.id);
      merged.push(item);
    }
  }
  return merged;
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

/** Fetches the main all-prompts source once for the application shell. */
export function usePrompts(
  refreshCounter: number,
  filter: PromptListFilter = 'all',
): PaginatedFetchState<PromptListItem> {
  const [prompts, setPrompts] = useState<PromptListItem[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const nextOffsetRef = useRef(0);
  const requestGenerationRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    const generation = ++requestGenerationRef.current;
    setLoading(true);
    setLoadingMore(false);
    setError(null);
    setPrompts([]);
    nextOffsetRef.current = 0;

    async function fetch() {
      try {
        const result = await api.prompts.list(filter, PAGE_SIZE, 0);

        if (!cancelled && generation === requestGenerationRef.current) {
          setPrompts(result);
          nextOffsetRef.current = PAGE_SIZE;
          setHasMore(result.length === PAGE_SIZE);
        }
      } catch (err) {
        if (!cancelled && generation === requestGenerationRef.current) {
          setError(asError(err));
          setHasMore(false);
        }
      } finally {
        if (!cancelled && generation === requestGenerationRef.current) {
          setLoading(false);
        }
      }
    }

    fetch();
    return () => {
      cancelled = true;
    };
  }, [filter, refreshCounter]);

  const loadMore = useCallback(() => {
    if (loading || loadingMore || !hasMore) return;
    const generation = requestGenerationRef.current;
    const offset = nextOffsetRef.current;
    setLoadingMore(true);
    setError(null);
    api.prompts
      .list(filter, PAGE_SIZE, offset)
      .then((page) => {
        if (generation !== requestGenerationRef.current) return;
        setPrompts((current) => mergePromptPages(current, page));
        nextOffsetRef.current = offset + PAGE_SIZE;
        setHasMore(page.length === PAGE_SIZE);
      })
      .catch((loadError) => {
        if (generation === requestGenerationRef.current) {
          setError(asError(loadError));
        }
      })
      .finally(() => {
        if (generation === requestGenerationRef.current) {
          setLoadingMore(false);
        }
      });
  }, [filter, hasMore, loading, loadingMore]);

  return {
    data: prompts,
    error,
    loading,
    hasMore,
    loadingMore,
    loadMore,
  };
}

export function useCollectionPrompts(
  collectionId: string | null,
  refreshCounter: number,
): PaginatedFetchState<PromptListItem> {
  const [prompts, setPrompts] = useState<PromptListItem[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const nextOffsetRef = useRef(0);
  const requestGenerationRef = useRef(0);

  useEffect(() => {
    const generation = ++requestGenerationRef.current;
    if (!collectionId) {
      setPrompts([]);
      setError(null);
      setLoading(false);
      setLoadingMore(false);
      setHasMore(false);
      nextOffsetRef.current = 0;
      return;
    }

    let cancelled = false;
    setLoading(true);
    setLoadingMore(false);
    setError(null);
    setPrompts([]);
    nextOffsetRef.current = 0;
    api.collections
      .getPrompts(collectionId, PAGE_SIZE, 0)
      .then((result) => {
        if (!cancelled && generation === requestGenerationRef.current) {
          setPrompts(result);
          nextOffsetRef.current = PAGE_SIZE;
          setHasMore(result.length === PAGE_SIZE);
        }
      })
      .catch((error) => {
        if (!cancelled && generation === requestGenerationRef.current) {
          setError(asError(error));
          setHasMore(false);
        }
      })
      .finally(() => {
        if (!cancelled && generation === requestGenerationRef.current) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [collectionId, refreshCounter]);

  const loadMore = useCallback(() => {
    if (!collectionId || loading || loadingMore || !hasMore) return;
    const generation = requestGenerationRef.current;
    const offset = nextOffsetRef.current;
    setLoadingMore(true);
    setError(null);
    api.collections
      .getPrompts(collectionId, PAGE_SIZE, offset)
      .then((page) => {
        if (generation !== requestGenerationRef.current) return;
        setPrompts((current) => mergePromptPages(current, page));
        nextOffsetRef.current = offset + PAGE_SIZE;
        setHasMore(page.length === PAGE_SIZE);
      })
      .catch((loadError) => {
        if (generation === requestGenerationRef.current) {
          setError(asError(loadError));
        }
      })
      .finally(() => {
        if (generation === requestGenerationRef.current) {
          setLoadingMore(false);
        }
      });
  }, [collectionId, hasMore, loading, loadingMore]);

  return {
    data: prompts,
    error,
    loading,
    hasMore,
    loadingMore,
    loadMore,
  };
}

export function usePromptCounts(
  refreshCounter: number,
): FetchState<PromptCounts | null> {
  const [counts, setCounts] = useState<PromptCounts | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api.prompts
      .counts()
      .then((result) => {
        if (!cancelled) setCounts(result);
      })
      .catch((fetchError) => {
        if (!cancelled) setError(asError(fetchError));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [refreshCounter]);

  return { data: counts, error, loading };
}

/**
 * Fetches a single prompt with its variants and tags.
 */
export function usePromptDetail(
  id: string | null,
  refreshCounter?: number,
): PromptDetailFetchState {
  const [prompt, setPrompt] = useState<PromptWithVariants | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshError, setRefreshError] = useState<Error | null>(null);
  const loadedPromptIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (!id) {
      loadedPromptIdRef.current = null;
      setPrompt(null);
      setError(null);
      setLoading(false);
      setRefreshing(false);
      setRefreshError(null);
      return;
    }

    let cancelled = false;
    const identityLoad = loadedPromptIdRef.current !== id;
    if (identityLoad) {
      setLoading(true);
      setRefreshing(false);
      setError(null);
    } else {
      setLoading(false);
      setRefreshing(true);
      setRefreshError(null);
    }

    api.prompts
      .get(id)
      .then((result) => {
        if (!cancelled) {
          loadedPromptIdRef.current = id;
          setPrompt(result);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          if (identityLoad) {
            setError(asError(err));
          } else {
            setRefreshError(asError(err));
          }
        }
      })
      .finally(() => {
        if (!cancelled) {
          if (identityLoad) {
            setLoading(false);
          } else {
            setRefreshing(false);
          }
        }
      });

    return () => { cancelled = true; };
  }, [id, refreshCounter]);

  return { data: prompt, error, loading, refreshing, refreshError };
}

/**
 * Fetches all tags.
 */
export function useTags(
  refreshCounter: number,
): FetchState<Tag[]> {
  const [tags, setTags] = useState<Tag[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api.tags
      .list()
      .then((result) => {
        if (!cancelled) setTags(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { data: tags, error, loading };
}

/**
 * Fetches all collections.
 */
export function useCollections(
  refreshCounter: number,
): FetchState<Collection[]> {
  const [collections, setCollections] = useState<Collection[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api.collections
      .list()
      .then((result) => {
        if (!cancelled) setCollections(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { data: collections, error, loading };
}

/**
 * Fetches all playbooks.
 */
export function usePlaybooks(
  refreshCounter: number,
): FetchState<Playbook[]> {
  const [playbooks, setPlaybooks] = useState<Playbook[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api.playbooks
      .list()
      .then((result) => {
        if (!cancelled) setPlaybooks(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { data: playbooks, error, loading };
}

export function usePlaybookDetail(
  id: string | null,
): FetchState<PlaybookWithSteps | null> {
  const [playbook, setPlaybook] = useState<PlaybookWithSteps | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!id) {
      setPlaybook(null);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    api.playbooks
      .get(id)
      .then((result) => {
        if (!cancelled) setPlaybook(result);
      })
      .catch((fetchError) => {
        if (!cancelled) setError(asError(fetchError));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [id]);

  return { data: playbook, error, loading };
}

/**
 * Fetches keyboard shortcuts.
 */
export function useKeyboardShortcuts(
  refreshCounter: number,
): FetchState<KeyboardShortcut[]> {
  const [shortcuts, setShortcuts] = useState<KeyboardShortcut[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api.settings
      .getShortcuts()
      .then((result) => {
        if (!cancelled) setShortcuts(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { data: shortcuts, error, loading };
}

/**
 * Debounced full-text search (300ms debounce, 2+ char minimum).
 */
export function useSearch(
  query: string,
  retryCounter = 0,
): FetchState<PromptListItem[]> {
  const [results, setResults] = useState<PromptListItem[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }

    if (query.length < 2) {
      setResults([]);
      setError(null);
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    timerRef.current = setTimeout(() => {
      api
        .search(query)
        .then((result) => {
          setResults(result);
        })
        .catch((err) => {
          setError(asError(err));
        })
        .finally(() => {
          setLoading(false);
        });
    }, 300);

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current);
      }
    };
  }, [query, retryCounter]);

  return { data: results, error, loading };
}

/**
 * Fetches the active playbook session.
 */
export function usePlaybookSession(
  refreshCounter: number,
): FetchState<PlaybookSession | null> {
  const [session, setSession] = useState<PlaybookSession | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    api.session
      .get()
      .then((result) => {
        if (!cancelled) setSession(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { data: session, error, loading };
}
