import { AppProvider } from './lib/context';
import { Sidebar } from './components/sidebar/Sidebar';

function PromptList() {
  return (
    <div
      className="flex-1 min-w-[280px] border-r overflow-y-auto"
      style={{
        borderColor: 'var(--border)',
        background: 'var(--bg-primary)',
      }}
    >
      <div className="p-6 text-sm" style={{ color: 'var(--text-secondary)' }}>
        Prompt list
      </div>
    </div>
  );
}

function DetailPanel() {
  return (
    <div
      className="flex-1 min-w-[320px] overflow-y-auto"
      style={{ background: 'var(--bg-secondary)' }}
    >
      <div className="p-6 text-sm" style={{ color: 'var(--text-secondary)' }}>
        Detail panel
      </div>
    </div>
  );
}

export default function App() {
  return (
    <AppProvider>
      <div
        className="flex h-screen overflow-hidden"
        style={{ background: 'var(--bg-primary)', color: 'var(--text-primary)' }}
      >
        <Sidebar />
        <PromptList />
        <DetailPanel />
      </div>
    </AppProvider>
  );
}
