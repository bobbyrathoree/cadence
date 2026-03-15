import { useAppContext } from '../../lib/context';
import { usePlaybooks } from '../../lib/hooks';
import { PromptDetail } from './PromptDetail';
import { NewPromptForm } from './NewPromptForm';
import { PlaybookStepper } from '../playbook/PlaybookStepper';
import { PlaybookBillboard } from '../playbook/PlaybookBillboard';

export function DetailPanel() {
  const { selectedPromptId, activeView, activePlaybookId, refreshCounter, isCreating } =
    useAppContext();
  const { playbooks } = usePlaybooks(refreshCounter);

  const showPlaybook = activeView === 'playbook';

  return (
    <div
      className="flex-1 min-w-[320px] overflow-hidden flex flex-col"
      style={{ background: 'var(--bg-secondary)' }}
    >
      {isCreating ? (
        <NewPromptForm />
      ) : showPlaybook && activePlaybookId ? (
        <PlaybookStepper playbookId={activePlaybookId} />
      ) : showPlaybook && playbooks.length === 0 ? (
        <PlaybookBillboard />
      ) : selectedPromptId ? (
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
