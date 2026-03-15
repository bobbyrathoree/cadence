import { useAppContext } from '../../lib/context';
import { PromptDetail } from './PromptDetail';

export function DetailPanel() {
  const { selectedPromptId } = useAppContext();

  return (
    <div
      className="flex-1 min-w-[320px] overflow-hidden flex flex-col"
      style={{ background: 'var(--bg-secondary)' }}
    >
      {selectedPromptId ? (
        <PromptDetail promptId={selectedPromptId} />
      ) : (
        <div
          className="flex-1 flex items-center justify-center"
          style={{ color: 'var(--text-secondary)', fontSize: '13px' }}
        >
          Select a prompt to view
        </div>
      )}
    </div>
  );
}
