import { AppProvider } from './lib/context';
import { Sidebar } from './components/sidebar/Sidebar';
import { PromptList } from './components/prompt-list/PromptList';
import { DetailPanel } from './components/detail/DetailPanel';

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
