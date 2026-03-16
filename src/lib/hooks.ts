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

/**
 * Fetches prompt list based on the active view.
 */
export function usePrompts(
  view: string,
  collectionId: string | null,
  refreshCounter: number,
): { prompts: PromptListItem[]; loading: boolean } {
  const [prompts, setPrompts] = useState<PromptListItem[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    async function fetch() {
      try {
        let result: PromptListItem[];

        if (view === 'collection' && collectionId) {
          result = await api.collections.getPrompts(collectionId);
        } else if (view === 'favorites') {
          const all = await api.prompts.list();
          result = all.filter((p) => p.is_favorite);
        } else if (view === 'recents') {
          const all = await api.prompts.list();
          result = all
            .filter((p) => p.last_copied_at !== null)
            .sort((a, b) => {
              const ta = a.last_copied_at ?? '';
              const tb = b.last_copied_at ?? '';
              return tb.localeCompare(ta);
            });
        } else {
          // 'all' or any default
          result = await api.prompts.list();
        }

        if (!cancelled) {
          setPrompts(result);
        }
      } catch (err) {
        console.error('usePrompts error:', err);
        if (!cancelled) {
          setPrompts([]);
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    fetch();
    return () => { cancelled = true; };
  }, [view, collectionId, refreshCounter]);

  return { prompts, loading };
}

/**
 * Fetches a single prompt with its variants and tags.
 */
export function usePromptDetail(
  id: string | null,
  refreshCounter?: number,
): { prompt: PromptWithVariants | null; loading: boolean } {
  const [prompt, setPrompt] = useState<PromptWithVariants | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!id) {
      setPrompt(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);

    api.prompts
      .get(id)
      .then((result) => {
        if (!cancelled) setPrompt(result);
      })
      .catch((err) => {
        console.error('usePromptDetail error:', err);
        if (!cancelled) setPrompt(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [id, refreshCounter]);

  return { prompt, loading };
}

/**
 * Fetches all tags.
 */
export function useTags(
  refreshCounter: number,
): { tags: Tag[]; loading: boolean } {
  const [tags, setTags] = useState<Tag[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    api.tags
      .list()
      .then((result) => {
        if (!cancelled) setTags(result);
      })
      .catch((err) => {
        console.error('useTags error:', err);
        if (!cancelled) setTags([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { tags, loading };
}

/**
 * Fetches all collections.
 */
export function useCollections(
  refreshCounter: number,
): { collections: Collection[]; loading: boolean } {
  const [collections, setCollections] = useState<Collection[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    api.collections
      .list()
      .then((result) => {
        if (!cancelled) setCollections(result);
      })
      .catch((err) => {
        console.error('useCollections error:', err);
        if (!cancelled) setCollections([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { collections, loading };
}

/**
 * Fetches all playbooks.
 */
export function usePlaybooks(
  refreshCounter: number,
): { playbooks: Playbook[]; loading: boolean } {
  const [playbooks, setPlaybooks] = useState<Playbook[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    api.playbooks
      .list()
      .then((result) => {
        if (!cancelled) setPlaybooks(result);
      })
      .catch((err) => {
        console.error('usePlaybooks error:', err);
        if (!cancelled) setPlaybooks([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { playbooks, loading };
}

/**
 * Fetches keyboard shortcuts.
 */
export function useKeyboardShortcuts(
  refreshCounter: number,
): { shortcuts: KeyboardShortcut[]; loading: boolean } {
  const [shortcuts, setShortcuts] = useState<KeyboardShortcut[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    api.settings
      .getShortcuts()
      .then((result) => {
        if (!cancelled) setShortcuts(result);
      })
      .catch((err) => {
        console.error('useKeyboardShortcuts error:', err);
        if (!cancelled) setShortcuts([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { shortcuts, loading };
}

/**
 * Debounced full-text search (300ms debounce, 2+ char minimum).
 */
export function useSearch(
  query: string,
): { results: PromptListItem[]; loading: boolean } {
  const [results, setResults] = useState<PromptListItem[]>([]);
  const [loading, setLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
    }

    if (query.length < 2) {
      setResults([]);
      setLoading(false);
      return;
    }

    setLoading(true);

    timerRef.current = setTimeout(() => {
      api
        .search(query)
        .then((result) => {
          setResults(result);
        })
        .catch((err) => {
          console.error('useSearch error:', err);
          setResults([]);
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
  }, [query]);

  return { results, loading };
}

/**
 * Fetches the active playbook session.
 */
export function usePlaybookSession(
  refreshCounter: number,
): { session: PlaybookSession | null; loading: boolean } {
  const [session, setSession] = useState<PlaybookSession | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    api.session
      .get()
      .then((result) => {
        if (!cancelled) setSession(result);
      })
      .catch((err) => {
        console.error('usePlaybookSession error:', err);
        if (!cancelled) setSession(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [refreshCounter]);

  return { session, loading };
}
