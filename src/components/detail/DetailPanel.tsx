import { useAppContext } from '../../lib/context';
import { usePlaybooks } from '../../lib/hooks';
import { PromptDetail } from './PromptDetail';
import { NewPromptForm } from './NewPromptForm';
import { PlaybookStepper } from '../playbook/PlaybookStepper';
import { PlaybookBillboard } from '../playbook/PlaybookBillboard';
import { PlaybookBuilder } from '../playbook/PlaybookBuilder';
import type { PromptListItem } from '../../lib/types';

export function DetailPanel({ prompts }: { prompts: PromptListItem[] }) {
  const {
    selectedPromptId,
    activeView,
    activePlaybookId,
    refreshCounter,
    isCreating,
    playbookBuilderMode,
  } = useAppContext();
  const { data: playbooks, error: playbooksError } =
    usePlaybooks(refreshCounter);

  const showPlaybook = activeView === 'playbook';

  return (
    <div
      className="flex-1 min-w-[320px] overflow-hidden flex flex-col"
      style={{ background: 'var(--bg-secondary)' }}
    >
      {isCreating ? (
        <NewPromptForm />
      ) : playbookBuilderMode ? (
        <PlaybookBuilder prompts={prompts} />
      ) : showPlaybook && activePlaybookId ? (
        <PlaybookStepper playbookId={activePlaybookId} />
      ) : showPlaybook && playbooksError ? (
        <div
          role="alert"
          className="flex-1 flex items-center justify-center"
          style={{ color: '#ff453a', fontSize: 13 }}
        >
          Couldn't load playbooks
        </div>
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
