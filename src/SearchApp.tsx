import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { FloatingSearch } from './components/search/FloatingSearch';

export function SearchApp() {
  // Global ESC handler to hide the search window
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
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
      <FloatingSearch />
    </div>
  );
}
