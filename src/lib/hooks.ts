import { useState, useEffect, useRef } from 'react';
import { api } from './api';
import type {
  PromptListItem,
  PromptWithVariants,
  Tag,
  Collection,
  Playbook,
  PlaybookSession,
  KeyboardShortcut,
} from './types';

export interface FetchState<T> {
  data: T;
  error: Error | null;
  loading: boolean;
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

/** Fetches the main all-prompts source once for the application shell. */
export function usePrompts(
  refreshCounter: number,
): FetchState<PromptListItem[]> {
  const [prompts, setPrompts] = useState<PromptListItem[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    async function fetch() {
      try {
        const result = await api.prompts.list();

        if (!cancelled) {
          setPrompts(result);
        }
      } catch (err) {
        if (!cancelled) setError(asError(err));
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetch();
    return () => {
      cancelled = true;
    };
  }, [refreshCounter]);

  return { data: prompts, error, loading };
}

export function useCollectionPrompts(
  collectionId: string | null,
  refreshCounter: number,
): FetchState<PromptListItem[]> {
  const [prompts, setPrompts] = useState<PromptListItem[]>([]);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!collectionId) {
      setPrompts([]);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    api.collections
      .getPrompts(collectionId)
      .then((result) => {
        if (!cancelled) setPrompts(result);
      })
      .catch((error) => {
        if (!cancelled) setError(asError(error));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [collectionId, refreshCounter]);

  return { data: prompts, error, loading };
}

/**
 * Fetches a single prompt with its variants and tags.
 */
export function usePromptDetail(
  id: string | null,
  refreshCounter?: number,
): FetchState<PromptWithVariants | null> {
  const [prompt, setPrompt] = useState<PromptWithVariants | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!id) {
      setPrompt(null);
      setError(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);

    api.prompts
      .get(id)
      .then((result) => {
        if (!cancelled) setPrompt(result);
      })
      .catch((err) => {
        if (!cancelled) setError(asError(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [id, refreshCounter]);

  return { data: prompt, error, loading };
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
