import { beforeEach, describe, expect, it, vi } from 'vitest';
import { api } from './api';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

describe('paginated IPC transport arguments', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.invoke.mockResolvedValue([]);
  });

  it('passes filter, limit, and offset to prompt and collection commands', async () => {
    await api.prompts.list('favorites', 100, 1_100);
    await api.collections.getPrompts('collection-1', 100, 200);
    await api.search('query', 100);

    expect(mocks.invoke).toHaveBeenNthCalledWith(1, 'list_prompts', {
      filter: 'favorites',
      limit: 100,
      offset: 1_100,
    });
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, 'get_collection_prompts', {
      collectionId: 'collection-1',
      limit: 100,
      offset: 200,
    });
    expect(mocks.invoke).toHaveBeenNthCalledWith(3, 'search_prompts', {
      query: 'query',
      limit: 100,
    });
  });

  it('uses the lifecycle commands for the local API setting', async () => {
    mocks.invoke
      .mockResolvedValueOnce(false)
      .mockResolvedValueOnce({ enabled: true, port: 41_237 });

    await api.settings.getApiEnabled();
    await api.settings.setApiEnabled(true);

    expect(mocks.invoke).toHaveBeenNthCalledWith(1, 'get_api_enabled');
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, 'set_api_enabled', {
      enabled: true,
    });
  });

  it('preserves null description clears across both IPC transports', async () => {
    mocks.invoke.mockResolvedValue(undefined);

    await api.prompts.update('prompt-1', { description: null });
    await api.playbooks.update('playbook-1', { description: null });

    expect(mocks.invoke).toHaveBeenNthCalledWith(1, 'update_prompt', {
      id: 'prompt-1',
      request: { description: null },
    });
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, 'update_playbook', {
      id: 'playbook-1',
      request: { description: null },
    });
  });
});
