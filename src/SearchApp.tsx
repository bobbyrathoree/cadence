import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { FloatingSearch } from './components/search/FloatingSearch';

export function SearchApp() {
  const [revision, setRevision] = useState(0);
  const [shownRevision, setShownRevision] = useState(0);
  const [fillActive, setFillActive] = useState(false);
  const fillActiveRef = useRef(false);
  fillActiveRef.current = fillActive;

  useEffect(() => {
    const unlistenDbChanged = listen('db-changed', () => {
      setRevision((current) => current + 1);
    });
    const unlistenSearchShown = listen('search-shown', () => {
      setRevision((current) => current + 1);
      setShownRevision((current) => current + 1);
    });

    return () => {
      unlistenDbChanged.then((unlisten) => unlisten());
      unlistenSearchShown.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const unlistenFocus = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) {
        if (fillActiveRef.current) return;
        invoke('hide_search_window').catch(console.error);
      }
    });
    return () => {
      unlistenFocus.then((unlisten) => unlisten());
    };
  }, []);

  // Global ESC handler to hide the search window
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        if (fillActiveRef.current) return;
        invoke('hide_search_window').catch(console.error);
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div
      style={{
        width: '100vw',
        height: '100vh',
        background: 'var(--bg-primary)',
        borderRadius: 12,
        overflow: 'hidden',
        border: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <FloatingSearch
        revision={revision}
        shownRevision={shownRevision}
        onFillActiveChange={setFillActive}
      />
    </div>
  );
}
